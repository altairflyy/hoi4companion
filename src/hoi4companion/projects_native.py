"""Qualified native project and breakthrough observations."""

def read_projects(reader):
    from .country_native import read_section
    return read_section(reader,'projects')
