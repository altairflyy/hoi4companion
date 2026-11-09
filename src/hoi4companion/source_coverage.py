"""Native editorial accounting; never a claim of live progress or source authenticity."""
from .backend_core import _request

def inventory_items(guide):
    validated=_request(0,'import_inventory',guide=guide)
    # Source-ID remapping deliberately mutates this validated inventory.
    return guide['source_inventory']['items'] if validated is not None else None

def coverage_report(guide,sources):
    return _request(0,'import_coverage',guide=guide,sources=sources)
