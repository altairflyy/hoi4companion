import io
import json
import os
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch


class DistributionPathsTests(unittest.TestCase):
    def test_source_keeps_existing_data_and_windows_bundle_uses_user_profile(self):
        from hoi4companion.paths import data_directory
        import sys
        with patch.dict(os.environ,{},clear=True),patch.object(sys,'frozen',False,create=True):
            self.assertEqual(data_directory(),Path(__file__).resolve().parents[1]/'.local')
        with TemporaryDirectory() as folder,patch.dict(os.environ,{'LOCALAPPDATA':folder},clear=True),patch.object(sys,'frozen',True,create=True):
            self.assertEqual(data_directory(),Path(folder)/'HOI4Companion')
            with patch.dict(os.environ,{'HOI4COMPANION_DATA_DIR':folder+'/isolated'}):
                self.assertEqual(data_directory(),Path(folder)/'isolated')

    def test_frozen_module_commands_use_the_console_worker_and_preserve_arguments(self):
        import sys
        from hoi4companion.paths import module_command
        with patch.object(sys,'frozen',True,create=True),patch.object(sys,'executable',r'C:/Bundle/HOI4 Companion.exe'):
            self.assertEqual(module_command('hoi4companion.overlay_bridge','--port','8766'),
                             [str(Path('C:/Bundle/Companion Runtime.exe')),'-m','hoi4companion.overlay_bridge','--port','8766'])
        with patch.object(sys,'frozen',False,create=True):
            self.assertEqual(module_command('yt_dlp','--version'),[sys.executable,'-m','yt_dlp','--version'])


class DistributionLauncherTests(unittest.TestCase):
    def test_non_companion_json_on_the_port_is_not_treated_as_a_running_service(self):
        from hoi4companion.desktop import probe
        with patch('hoi4companion.desktop.urlopen',return_value=io.BytesIO(b'[]')):
            self.assertFalse(probe('http://127.0.0.1:8876'))

    def test_worker_dispatch_is_allowlisted_and_preserves_binary_stdio(self):
        from hoi4companion.desktop import dispatch_worker
        original=io.BytesIO(b'protocol')
        with patch('hoi4companion.desktop.runpy.run_module') as run,patch('sys.stdin',original):
            dispatch_worker(['-m','hoi4companion.overlay_bridge','--port','8766'])
            self.assertEqual(run.call_args.args,('hoi4companion.overlay_bridge',))
            import sys
            self.assertIs(sys.stdin,original)
        with self.assertRaises(ValueError):dispatch_worker(['-m','os'])

    def test_launcher_refuses_an_occupied_foreign_port_without_spawning(self):
        from hoi4companion.desktop import launch
        with TemporaryDirectory() as folder,patch('hoi4companion.desktop.data_directory',return_value=Path(folder)),\
             patch('hoi4companion.desktop.probe',return_value=False),\
             patch('hoi4companion.desktop.port_open',return_value=True),\
             patch('hoi4companion.desktop.subprocess.Popen') as spawn:
            with self.assertRaisesRegex(RuntimeError,'occupata'):launch(port=8876,browser=False,hud=False)
            spawn.assert_not_called()

    def test_repeated_launch_reuses_server_and_hud_remains_optional(self):
        from hoi4companion.desktop import launch
        with TemporaryDirectory() as folder,patch('hoi4companion.desktop.data_directory',return_value=Path(folder)),\
             patch('hoi4companion.desktop.probe',return_value=True),\
             patch('hoi4companion.desktop.subprocess.Popen') as spawn,\
             patch('hoi4companion.desktop.webbrowser.open') as browser:
            launch(port=8876,browser=False,hud=False)
            spawn.assert_not_called();browser.assert_not_called()


if __name__=='__main__':unittest.main()
