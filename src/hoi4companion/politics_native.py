"""Qualified native laws, ideas and active appointments."""
import json
from pathlib import Path
_PROFILE=json.loads(Path(__file__).with_name('country-functions-v1.json').read_bytes())
CURRENT_IDEA_SIGNATURES=tuple((r,bytes.fromhex(s)) for r,s in _PROFILE['ideas'])
CURRENT_ADVISOR_SIGNATURES=tuple((r,bytes.fromhex(s)) for r,s in _PROFILE['advisors'])


def read_politics(reader):
    from .country_native import read_section
    return read_section(reader,'politics')
