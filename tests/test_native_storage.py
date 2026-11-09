"""SQLite compatibility and conflict guards across native/legacy writers."""
import json
from pathlib import Path
import sqlite3
from tempfile import TemporaryDirectory
import unittest

from hoi4companion.storage_core import request,engine_info
from hoi4companion._progress_storage_reference import ProgressDatabase as Legacy


class NativeStorageTests(unittest.TestCase):
    def test_legacy_archive_roundtrip_and_conflicting_writer(self):
        self.assertEqual(engine_info()['engine'],'rust-storage')
        with TemporaryDirectory() as folder:
            path=Path(folder)/'progress.sqlite3';legacy=Legacy(path)
            value={'schema_version':1,'runs':{'é':{'checks':{'flag':False},'baselines':{'x':7.5}}},'active':{},'campaign_active':{}}
            legacy.save(value)
            row=request('load',path);self.assertEqual(json.loads(row['payload']),value);self.assertEqual(row['revision'],1)
            self.assertEqual(request('save',path,expected_revision=1,payload=json.dumps(value))['revision'],2)
            self.assertEqual(legacy.load(),value);self.assertEqual(legacy.revision,2)
            before=path.read_bytes()
            with self.assertRaisesRegex(ValueError,'conflict'):request('save',path,expected_revision=1,payload='{}')
            self.assertEqual(path.read_bytes(),before)

    def test_empty_nested_path_corruption_and_future_storage_version(self):
        with TemporaryDirectory() as folder:
            path=Path(folder)/'nested'/'progress.sqlite3'
            request('init',path);self.assertIsNone(request('load',path))
            request('save',path,expected_revision=0,payload='{}')
            with sqlite3.connect(path) as database:database.execute('UPDATE progress SET version=2 WHERE id=1')
            database.close()
            before=path.read_bytes()
            with self.assertRaisesRegex(ValueError,'version'):request('load',path)
            self.assertEqual(path.read_bytes(),before)
            with self.assertRaisesRegex(ValueError,'version'):
                request('save',path,expected_revision=1,payload='{"replacement":true}')
            self.assertEqual(path.read_bytes(),before)
            path=Path(folder)/'bad';path.write_bytes(b'not-a-database')
            with self.assertRaises(ValueError):request('init',path)
            self.assertEqual(path.read_bytes(),b'not-a-database')

    def test_bad_json_never_changes_the_persisted_snapshot(self):
        with TemporaryDirectory() as folder:
            path=Path(folder)/'progress.sqlite3';request('init',path)
            request('save',path,expected_revision=0,payload='{"ok":true}')
            for bad in ('{','NaN','{"value":Infinity}'):
                before=path.read_bytes()
                with self.assertRaises(ValueError):request('save',path,expected_revision=1,payload=bad)
                self.assertEqual(path.read_bytes(),before)
