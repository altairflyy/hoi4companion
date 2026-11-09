"""An older ABI-compatible DLL must not silently produce empty catalogs."""
import unittest
from hoi4companion.backend_core import engine_info

class BackendCapabilityTests(unittest.TestCase):
    def test_loaded_binary_declares_the_owned_source_generation_contract(self):
        info=engine_info()
        self.assertEqual(info.get('source_catalogs'),1)
        self.assertEqual(info.get('catalog_generation'),1)
        self.assertEqual(info.get('read_only'),True)
