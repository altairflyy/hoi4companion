"""Qualified native design decoder; unavailable designs retain explicit evidence."""
import json
from pathlib import Path
_PROFILE=json.loads(Path(__file__).with_name('design-profile-v1.json').read_bytes())
SIGNATURES=tuple((r,bytes.fromhex(b)) for r,b in _PROFILE['signatures'])
TOKEN_INITIALIZED=0x368F570
TOKEN_MAXIMUM=0x368F574
TOKEN_TABLE=0x368F5A0
MAX_ENTRIES=256


def read_design(reader,variant):
    from .country_native import read_section
    return read_section(reader,'design',variant=variant)
