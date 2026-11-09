"""Local corrections through Rust; guide sources and campaign progress stay separate."""
from pathlib import Path
from .paths import data_directory
from .backend_core import _request
DIRECTORY=data_directory()/'guide-associations'

def read_associations(guide_id,directory=None):
    return _request(0,'guide_associations_read',guide_id=guide_id,directory=str(Path(directory)if directory is not None else DIRECTORY))

def apply_associations(guide,directory=None):
    return _request(0,'guide_associations_apply',guide=guide,directory=str(Path(directory)if directory is not None else DIRECTORY))

def change_association(guide,key,expected,entity,catalog,directory=None):
    from .resolution import catalog_index
    index=catalog_index(catalog)
    return _request(0,'guide_association_change',guide=guide,key=key,expected=expected,entity=entity,directory=str(Path(directory)if directory is not None else DIRECTORY),**index.native_import_args())
