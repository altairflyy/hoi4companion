"""Qualified native experience and war state; external read-only transport."""
import struct
from .live import LiveReadError


def read_experience(reader):
    from .country_native import read_section
    return read_section(reader,'experience')


def read_diplomacy(reader):
    from .country_native import read_section
    return read_section(reader,'diplomacy')
