import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from hoi4companion.installation import active_mods, inspect_mods, default_documents
from hoi4companion.api import Store


class ModProfileTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.docs = self.root / 'profile'
        (self.docs / 'mod').mkdir(parents=True)

    def select(self, *names, docs=None):
        ((docs or self.docs) / 'dlc_load.json').write_text(json.dumps({'enabled_mods': list(names), 'disabled_dlcs': []}), encoding='utf-8')

    def descriptor(self, name, path, docs=None):
        file = (docs or self.docs) / 'mod' / (name + '.mod')
        file.write_text(f'name="{name}" path="{path}"', encoding='utf-8')
        return 'mod/' + file.name

    def test_launcher_relative_and_descriptor_local_paths(self):
        folder = self.docs / 'mod' / 'local'
        folder.mkdir()
        for path in ('mod/local', 'local'):
            self.select(self.descriptor('local', path))
            roots, warnings = active_mods(self.docs)
            self.assertEqual(roots, [folder.resolve()])
            self.assertEqual(warnings, [])
        (self.docs / 'local').mkdir()
        result = inspect_mods(self.docs)
        self.assertEqual(result['status'], 'partial')
        self.assertEqual(result['roots'], [])
        self.assertIn('ambiguo', result['warnings'][0])

    def test_reloads_profile_order_and_never_reuses_disabled_or_other_users_mods(self):
        folders = [self.root / name for name in ('one', 'two', 'disabled')]
        for folder in folders:
            folder.mkdir()
        refs = [self.descriptor(folder.name, folder.as_posix()) for folder in folders]
        self.select(refs[0], refs[1], refs[0])
        self.assertEqual(active_mods(self.docs)[0], [folders[1], folders[0]])
        other = self.root / 'other-profile'
        (other / 'mod').mkdir(parents=True)
        self.select(docs=other)
        self.assertEqual(active_mods(other), ([], []))
        self.select(refs[1])
        self.assertEqual(active_mods(self.docs), ([folders[1]], []))
        self.select()
        self.assertEqual(inspect_mods(self.docs)['status'], 'observed')
        self.assertEqual(active_mods(self.docs), ([], []))

    def test_missing_corrupt_profile_and_unreadable_mods_are_explicit(self):
        config = self.docs / 'dlc_load.json'
        self.assertEqual(inspect_mods(self.docs)['status'], 'unavailable')
        for text in ('null', '{', '{"enabled_mods":"mod/a.mod"}', '{}'):
            config.write_text(text, encoding='utf-8')
            self.assertEqual(inspect_mods(self.docs)['status'], 'unavailable')
            self.assertTrue(active_mods(self.docs)[1])
        (self.docs / 'mod' / 'zip.mod').write_text('archive="mod/a.zip"', encoding='utf-8')
        (self.docs / 'mod' / 'broken.mod').write_text('x={', encoding='utf-8')
        self.select('mod/missing.mod', self.descriptor('old-path', (self.root / 'missing').as_posix()),
                    'mod/zip.mod', 'mod/broken.mod', None)
        value = inspect_mods(self.docs)
        self.assertEqual(value['status'], 'partial')
        self.assertEqual(len(value['warnings']), 5)
        self.assertEqual(value['roots'], [])

    def test_store_refresh_changes_mod_catalog_and_reports_actual_source(self):
        game = self.root / 'game'
        (game / 'common' / 'national_focus').mkdir(parents=True)
        (game / 'common' / 'national_focus' / 'base.txt').write_text('focus_tree={id=base default=yes country={factor=1} focus={id=start}}', encoding='utf-8')
        folder = self.docs / 'mod' / 'local'
        (folder / 'common' / 'national_focus').mkdir(parents=True)
        (folder / 'common' / 'national_focus' / 'extra.txt').write_text('focus_tree={id=extra country={factor=0 modifier={add=10 tag=GER}} focus={id=extra_start}}', encoding='utf-8')
        self.select(self.descriptor('local', 'mod/local'))
        settings = self.root / 'settings.json'
        with patch('hoi4companion.api.SETTINGS', settings):
            store = Store({'game_dir': str(game), 'docs_dir': str(self.docs), 'mods': 'auto'})
            self.assertIn('extra', store.trees)
            self.assertEqual(store.overview()['mods']['roots'], [str(folder.resolve())])
            self.assertEqual(store.overview()['mods']['source'], str(self.docs / 'dlc_load.json'))
            self.select()
            store.refresh()
            self.assertNotIn('extra', store.trees)
            self.assertEqual(store.mod_roots, [])
            self.assertEqual(store.overview()['mods']['items'], [])
            store.set_settings({'mods': 'off'})
            self.assertEqual(store.overview()['mods']['status'], 'off')

    def test_persisted_custom_profile_wins_over_discovery(self):
        settings = self.root / 'settings.json'
        settings.write_text(json.dumps({'game_dir': str(self.root / 'custom-game'), 'docs_dir': str(self.docs)}), encoding='utf-8')
        with patch('hoi4companion.api.SETTINGS', settings), patch.object(Store, 'refresh'), \
                patch('hoi4companion.api.detect_game') as detect, patch('hoi4companion.api.default_documents') as docs:
            store = Store()
            self.assertEqual(store.settings['docs_dir'], str(self.docs))
            detect.assert_not_called()
            docs.assert_not_called()

    def test_fresh_profile_discovers_own_paths_and_explicit_paths_win(self):
        settings = self.root / 'settings.json'
        with patch('hoi4companion.api.SETTINGS', settings), patch.object(Store, 'refresh'), \
                patch('hoi4companion.api.detect_game', return_value=self.root / 'found-game'), \
                patch('hoi4companion.api.default_documents', return_value=self.docs):
            store = Store()
            self.assertEqual(store.settings['game_dir'], str(self.root / 'found-game'))
            self.assertEqual(store.settings['docs_dir'], str(self.docs))
            store = Store({'docs_dir': str(self.root / 'explicit-profile')})
            self.assertEqual(store.settings['docs_dir'], str(self.root / 'explicit-profile'))

    def test_cli_passes_only_explicit_paths_to_preserve_saved_profile(self):
        from hoi4companion.__main__ import main
        with patch('sys.argv', ['hoi4companion', '--no-browser']), \
                patch('hoi4companion.__main__.Store') as factory, patch('hoi4companion.__main__.serve'):
            main()
            factory.assert_called_once_with({})

    def test_windows_redirected_documents_uses_current_user_location(self):
        if os.name != 'nt':
            return  # Windows registry is not available on other platforms.
        with patch('winreg.OpenKey'), patch('winreg.QueryValueEx', return_value=(str(self.root / 'redirected'), 1)):
            self.assertEqual(default_documents(), self.root / 'redirected' / 'Paradox Interactive' / 'Hearts of Iron IV')
