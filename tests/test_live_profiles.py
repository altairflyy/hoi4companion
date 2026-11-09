from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.live_profiles import profiles
from hoi4companion._live_reference import EXECUTABLE_PROFILES as reference_profiles
from hoi4companion.live import ExecutableProfile,LiveReadError


class LiveProfilesTests(unittest.TestCase):
    def test_unknown_profile_handles_are_rejected_at_the_native_boundary(self):
        self.assertFalse(_request(0,'profile_release',handle=2**64-1))
        with self.assertRaisesRegex(ValueError,'Profilo eseguibile non disponibile'):
            _request(0,'profile_class',handle=2**64-1,rva=0)

    def test_shared_manifest_preserves_every_existing_qualified_layout(self):
        self.assertEqual(profiles(),reference_profiles)

    def test_unknown_executable_and_mismatched_profile_manifest_are_rejected(self):
        with TemporaryDirectory() as root:
            path=Path(root)/'hoi4.exe';path.write_bytes(b'unknown executable')
            with self.assertRaisesRegex(LiveReadError,'Versione HOI4'):ExecutableProfile(path)
            with self.assertRaisesRegex(ValueError,'Profili live modificati'):
                _request(0,'profile_open',path=str(path),manifest_sha256='0'*64)
            self.assertEqual(path.read_bytes(),b'unknown executable')
