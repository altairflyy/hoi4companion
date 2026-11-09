"""Native developer export retains Jomini encodings and duplicate policy."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]

class NativeScriptExportTests(unittest.TestCase):
    def test_export_encoding_duplicates_and_exclusive_destination(self):
        executable=os.environ.get('HOI4_BACKEND_EXE',str(ROOT/'src/hoi4companion/_native/hoi4-backend.exe'))
        with tempfile.TemporaryDirectory() as directory:
            source=Path(directory)/'source.txt';output=Path(directory)/'export.json'
            source.write_bytes('name="café" value=one value=two'.encode('cp1252'))
            command=[executable,'--export-script',str(source),'--script-encoding','windows-1252','--json-duplicates','group','--output',str(output)]
            run=subprocess.run(command,capture_output=True,timeout=15)
            self.assertEqual(run.returncode,0,run.stderr)
            value=json.loads(output.read_text(encoding='utf8'))
            self.assertEqual(value['name'],'café');self.assertEqual(value['value'],['one','two'])
            original=output.read_bytes()
            self.assertNotEqual(subprocess.run(command,capture_output=True,timeout=15).returncode,0)
            self.assertEqual(output.read_bytes(),original)
            self.assertEqual(source.read_bytes(),'name="café" value=one value=two'.encode('cp1252'))
