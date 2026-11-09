"""Owned Rust catalog handles. No silent Python resolver fallback or workers."""
import ctypes
from importlib.machinery import EXTENSION_SUFFIXES
import json
import os
from pathlib import Path
import sys
from threading import RLock
import weakref

MAX_BYTES=64*1024*1024
_lock=RLock()
_dll=None


class CatalogCoreUnavailable(ValueError):
    pass


def _library():
    global _dll
    with _lock:
        if _dll is not None:return _dll
        override=os.environ.get('HOI4_CATALOG_CORE_LIBRARY')
        folder=Path(__file__).parent/'_native'
        suffix='.dll' if sys.platform=='win32' else '.dylib' if sys.platform=='darwin' else '.so'
        candidates=[Path(override)] if override else [folder/('hoi4companion_catalog_core'+x) for x in dict.fromkeys([suffix,*EXTENSION_SUFFIXES])]
        path=next((p for p in candidates if p.is_file()),None)
        if path is None:raise CatalogCoreUnavailable('Rust catalog core is not installed. Run scripts/build-catalog-core.ps1.')
        try:
            dll=ctypes.CDLL(str(path.resolve()))
            dll.hoi4_catalog_abi_version.argtypes=[];dll.hoi4_catalog_abi_version.restype=ctypes.c_uint32
            if dll.hoi4_catalog_abi_version()!=1:raise ValueError('Unsupported catalog core ABI')
            for name,args in [('hoi4_catalog_info',[]),('hoi4_catalog_compile',[ctypes.c_void_p,ctypes.c_size_t]),
                              ('hoi4_catalog_query',[ctypes.c_uint64,ctypes.c_void_p,ctypes.c_size_t])]:
                fn=getattr(dll,name);fn.argtypes=args;fn.restype=ctypes.c_void_p
            dll.hoi4_catalog_release.argtypes=[ctypes.c_uint64];dll.hoi4_catalog_release.restype=ctypes.c_bool
            dll.hoi4_catalog_free.argtypes=[ctypes.c_void_p];dll.hoi4_catalog_free.restype=None
        except (OSError,ValueError,AttributeError) as exc:raise CatalogCoreUnavailable('Rust catalog core could not be loaded: '+str(exc)) from exc
        _dll=dll
        return dll


def _reply(dll,ptr):
    if not ptr:raise CatalogCoreUnavailable('Rust catalog core returned no result')
    try:value=json.loads(ctypes.string_at(ptr))
    finally:dll.hoi4_catalog_free(ptr)
    if not isinstance(value,dict) or value.get('schema_version')!=1:raise CatalogCoreUnavailable('Invalid catalog core response')
    if 'error' in value:raise ValueError('Rust catalog core: '+str(value['error']))
    if 'result' not in value:raise CatalogCoreUnavailable('Incomplete catalog core response')
    return value['result']


def _encode(value):
    data=json.dumps(value,ensure_ascii=False,separators=(',',':'),allow_nan=False).encode('utf8')
    if len(data)>MAX_BYTES:raise ValueError('Native catalog input exceeds limit')
    return data


def engine_info():
    from .backend_core import _request
    return _request(0,'identity_info')


class NativeCatalog:
    def __init__(self,entries,*,country_names=None,adjectives=()):
        from .research_upgrades import guide_language
        from .backend_core import _request
        self._lock=RLock()
        self._handle=_request(0,'identity_open',schema_version=1,entries=list(entries),
                              country_names={key:sorted(values) for key,values in (country_names or {}).items()},
                              adjectives=list(adjectives),vocabulary=guide_language())['handle']
        self._finalizer=weakref.finalize(self,_request,0,'identity_release',handle=self._handle)

    def close(self):
        with self._lock:
            self._finalizer()
            self._handle=None

    def __enter__(self):return self
    def __exit__(self,*args):self.close()

    def query(self,op,**kwargs):
        from .backend_core import _request
        with self._lock:
            if self._handle is None:raise ValueError('Native catalog is closed')
            return _request(0,'identity_query',handle=self._handle,request={'schema_version':1,'op':op,**kwargs})
