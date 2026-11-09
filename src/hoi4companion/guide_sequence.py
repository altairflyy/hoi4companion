"""Native structural barrier diagnostics, separate from run advancement."""
from .backend_core import _request

def sequence_report(guide):
    return _request(0,'import_sequence',guide=guide)
