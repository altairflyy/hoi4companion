"""Qualified native provincial building levels and controllers."""

def read(reader):
    from .country_native import read_section
    return read_section(reader,'fortifications')
