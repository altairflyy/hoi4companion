"""Load the archived comparison parser without placing it in the runtime package."""
import importlib.util
from pathlib import Path
spec = importlib.util.spec_from_file_location(
    'legacy_clausewitz', Path(__file__).resolve().parents[1] / 'scripts' / 'legacy_clausewitz.py')
reference = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reference)
parse_legacy = reference.parse_legacy
