"""Native transactional storage adapter; SQLite format v1 stays unchanged."""
import ctypes
from importlib.machinery import EXTENSION_SUFFIXES
import json
import os
from pathlib import Path
import sys
from threading import RLock

_dll=None
_lock=RLock()


class StorageCoreUnavailable(ValueError):
    pass


def _library():
    global _dll
    with _lock:
        if _dll is not None:return _dll
        folder=Path(__file__).parent/'_native';override=os.environ.get('HOI4_STORAGE_LIBRARY')
        suffix='.dll' if sys.platform=='win32' else '.dylib' if sys.platform=='darwin' else '.so'
        paths=[Path(override)] if override else [folder/('hoi4companion_storage'+s) for s in dict.fromkeys([suffix,*EXTENSION_SUFFIXES])]
        path=next((p for p in paths if p.is_file()),None)
        if path is None:raise StorageCoreUnavailable('Rust storage is not installed. Run scripts/build-storage.ps1.')
        try:
            dll=ctypes.CDLL(str(path.resolve()))
            dll.hoi4_storage_abi_version.argtypes=[];dll.hoi4_storage_abi_version.restype=ctypes.c_uint32
            if dll.hoi4_storage_abi_version()!=1:raise ValueError('Unsupported storage ABI')
            dll.hoi4_storage_info.argtypes=[];dll.hoi4_storage_info.restype=ctypes.c_void_p
            dll.hoi4_storage_request.argtypes=[ctypes.c_void_p,ctypes.c_size_t];dll.hoi4_storage_request.restype=ctypes.c_void_p
            dll.hoi4_storage_free.argtypes=[ctypes.c_void_p];dll.hoi4_storage_free.restype=None
        except (OSError,ValueError,AttributeError) as exc:raise StorageCoreUnavailable('Rust storage could not be loaded: '+str(exc)) from exc
        _dll=dll
        return dll


def _reply(dll,ptr):
    if not ptr:raise StorageCoreUnavailable('Rust storage returned no result')
    try:value=json.loads(ctypes.string_at(ptr))
    finally:dll.hoi4_storage_free(ptr)
    if value.get('schema_version')!=1:raise StorageCoreUnavailable('Invalid storage response')
    if 'error' in value:raise ValueError('Rust storage: '+str(value['error']))
    return value['result']


def engine_info():
    dll=_library();return _reply(dll,dll.hoi4_storage_info())


def request(op,path,**kwargs):
    data=json.dumps({'schema_version':1,'op':op,'path':str(Path(path).absolute()),**kwargs},ensure_ascii=False,allow_nan=False).encode('utf8')
    if len(data)>96*1024*1024:raise ValueError('Storage input exceeds limit')
    dll=_library();buffer=ctypes.create_string_buffer(data)
    return _reply(dll,dll.hoi4_storage_request(buffer,len(data)))
