"""Compact native AI projection; localized resolver catalogs remain unchanged."""

def for_ai(entries, reference=None, localized=None):
    from .backend_core import _request
    return _request(0,'source_export',entries=list(entries),reference=reference or {})
