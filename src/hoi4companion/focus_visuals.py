"""Read focus GUI geometry; no gameplay conditions or process access.

Use the same read-only parser and texture-header reader as technology visuals.
Definitions follow installation/mod file precedence. Missing metrics stay absent.
"""
from pathlib import Path
import re

from .jomini_parser import parse_file
from .technology_visuals import _field, _number, _point, _walk, _dimensions

POSITIONS = ('focus_spacing', 'national_focus_center', 'link_spacing', 'link_offsets',
             'link_begin', 'link_end', 'exclusive_offset', 'exclusive_offset_left',
             'exclusive_positioning')
TEMPLATES = ('national_focus_item', 'national_focus_link', 'national_focus_exclusive_item')


def load_focus_visuals(gui_files,sprite_library=None):
    from .backend_core import _request
    handle=getattr(sprite_library,'_handle',None)
    if sprite_library is not None and handle is None:raise ValueError('Native sprite generation required')
    return _request(0,'source_focus_visuals',files=list(map(str,gui_files)),assets_handle=handle)
