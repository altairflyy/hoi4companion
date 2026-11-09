"""Synthetic structure faults; real-build evidence is documented separately."""
import hashlib
import unittest
from unittest.mock import patch

from hoi4companion.country import CountryReader
from hoi4companion.doctrines_native import _FUNCTIONS, read_doctrines
from hoi4companion.live import MILITARY_HASH, LiveReadError, compile_watch_reads, watch_changed
from test_native_fixture import Fixture


class DoctrineFixture(Fixture):
    def __init__(self):
        super().__init__()
        self.profile.sha256 = MILITARY_HASH
        self.profile.world_rva = 0x33DD6B0
        self.profile.bytes = lambda rva, size: b'\x90' * size
        self.put(self.base + self.profile.world_rva, '<Q', self.world)
        self.system = self.object('CDoctrineSystem@NDoctrines')
        self.put(self.world + 0x400, '<Q', self.system)
        self.status = self.object('CCountryDoctrineStatus@NDoctrines')
        self.put(self.system + 8, '<Qii', self.status, 1, 1)
        self.put(self.status + 8, '<i', 1)
        self.folder = self.object('CFolderStatus@NDoctrines')
        self.put(self.status + 0x10, '<Qii', self.folder, 1, 1)
        self.template = self.object('CFolderTemplate@NDoctrines')
        self.put(self.folder + 8, '<Q', self.template)
        self.grand = self.object('CGrandDoctrineTemplate@NDoctrines')
        self.put(self.folder + 0x10, '<Q', self.grand)
        self.track = self.object('CTrackStatus@NDoctrines')
        self.put(self.folder + 0x18, '<Qii', self.track, 1, 1)
        self.sub = self.object('CSubDoctrineTemplate@NDoctrines')
        self.put(self.track + 8, '<Q', self.sub)
        self.atoms = self.allocate()
        self.put(self.base + 0x368F570, '<B', 1)
        self.put(self.base + 0x368F574, '<i', 10)
        self.put(self.base + 0x368F5A0, '<Q', self.atoms)
        self.put(self.grand + 8, '<i', 1)
        self.put(self.sub + 8, '<i', 2)
        self.string(self.atoms + 0x20, 'grand_battleplan')
        self.string(self.atoms + 0x40, 'grand_assault')

    def reader(self):
        return CountryReader(self, self.world, self.country, 1)


class DoctrineTests(unittest.TestCase):
    def setUp(self):
        self.signatures = patch('hoi4companion.doctrines_native._FUNCTIONS', tuple(
            (rva, size, hashlib.sha256(b'\x90' * size).hexdigest())
            for rva, size, _ in _FUNCTIONS))
        self.signatures.start()
        self.addCleanup(self.signatures.stop)

    def test_selected_grand_and_subdoctrine_membership(self):
        f = DoctrineFixture(); r = f.reader()
        self.assertEqual(read_doctrines(r), {'status': 'observed', 'items': [
            {'id': 'grand_battleplan'}, {'id': 'grand_assault'}]})
        r.s.verify()

    def test_empty_selection_is_observed_and_available_template_is_not_selected(self):
        f = DoctrineFixture()
        f.put(f.folder + 0x10, '<Q', 0)
        f.put(f.folder + 0x18, '<Qii', 0, 0, 0)
        self.assertEqual(read_doctrines(f.reader()), {'status': 'observed', 'items': []})
        f.put(f.status + 0x10, '<Qii', 0, 0, 0)
        self.assertEqual(read_doctrines(f.reader()), {'status': 'observed', 'items': []})

    def test_signature_and_unsupported_build_fail_closed(self):
        for mutation in (
                lambda f: setattr(f.profile, 'sha256', 'other'),
                lambda f: setattr(f.profile, 'bytes', lambda rva, size: b'\0' * size)):
            f = DoctrineFixture(); mutation(f)
            self.assertEqual(read_doctrines(f.reader())['status'], 'unavailable')

    def test_country_owner_mapping_and_session_cannot_fall_back(self):
        for mutation in (
                lambda f: f.put(f.status + 8, '<i', 2),
                lambda f: f.array(f.world + 0x340, [0, -1], 4),
                lambda f: f.array(f.world + 0x340, [0, 2], 4),
                lambda f: f.array(f.world + 0x310, [f.world]),
                lambda f: f.put(f.system + 8, '<Qii', 0, 0, 0),
                lambda f: f.put(f.country + 8, '<i', 2),
                lambda f: f.put(f.base + f.profile.world_rva, '<Q', f.country)):
            f = DoctrineFixture(); mutation(f)
            value = read_doctrines(f.reader())
            self.assertEqual(value['status'], 'unavailable')
            self.assertNotIn('items', value)

    def test_container_type_token_and_duplicate_faults_publish_no_prefix(self):
        for mutation in (
                lambda f: f.put(f.folder + 0x18, '<Qii', 0, 1, 1),
                lambda f: f.put(f.status + 0x10, '<Qii', f.folder, 1, 2),
                lambda f: f.put(f.folder + 0x18, '<Qii', f.track, 257, 257),
                lambda f: f.put(f.sub, '<Q', 0),
                lambda f: f.put(f.sub + 8, '<i', 11),
                lambda f: f.put(f.sub + 8, '<i', 1),
                lambda f: f.put(f.base + 0x368F570, '<B', 0),
                lambda f: f.put(f.folder + 0x10, '<Q', 0)):
            f = DoctrineFixture(); mutation(f)
            value = read_doctrines(f.reader())
            self.assertEqual(value['status'], 'unavailable')
            self.assertNotIn('items', value)

    def test_selection_and_ownership_changes_invalidate_watch_and_sample(self):
        for field in ('owner', 'selection', 'folders', 'token', 'table'):
            f = DoctrineFixture(); r = f.reader()
            self.assertEqual(read_doctrines(r)['status'], 'observed')
            r.s.verify(); plan = compile_watch_reads(r.watch)
            address = {'owner': f.status + 8, 'selection': f.track + 8,
                       'folders': f.status + 0x1C, 'token': f.sub + 8,
                       'table': f.base + 0x368F5A0}[field]
            f.put(address, '<i', 0)
            self.assertTrue(watch_changed(f, plan), field)
            with self.assertRaises(LiveReadError):
                r.s.verify()


if __name__ == '__main__':
    unittest.main()
