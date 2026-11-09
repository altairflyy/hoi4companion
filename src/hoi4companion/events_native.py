"""Passive native event popup and decision observations; no automatic choices."""
GUI_RVA=0x34531E0


def read_events(reader):
    from .country_native import read_section
    return read_section(reader,'events')


def read_decisions(reader):
    from .country_native import read_section
    return read_section(reader,'decisions')
