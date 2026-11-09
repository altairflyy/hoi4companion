"""C ABI for the owned Rust application services; no fallback interpreter."""
import ctypes
from importlib.machinery import EXTENSION_SUFFIXES
import json
import os
from pathlib import Path
import sys
from threading import RLock
import weakref

_dll=None
_lock=RLock()


class BackendUnavailable(ValueError):pass


def _library():
    global _dll
    with _lock:
        if _dll is not None:return _dll
        folder=Path(__file__).parent/'_native';override=os.environ.get('HOI4_BACKEND_LIBRARY')
        suffix='.dll' if sys.platform=='win32' else '.dylib' if sys.platform=='darwin' else '.so'
        paths=[Path(override)] if override else [folder/('hoi4companion_backend'+s) for s in dict.fromkeys([suffix,*EXTENSION_SUFFIXES])]
        path=next((p for p in paths if p.is_file()),None)
        if path is None:raise BackendUnavailable('Rust backend services missing. Run scripts/build-backend.ps1.')
        try:
            dll=ctypes.CDLL(str(path.resolve()))
            dll.hoi4_backend_abi_version.argtypes=[];dll.hoi4_backend_abi_version.restype=ctypes.c_uint32
            if dll.hoi4_backend_abi_version()!=1:raise ValueError('Unsupported backend ABI')
            dll.hoi4_backend_info.argtypes=[];dll.hoi4_backend_info.restype=ctypes.c_void_p
            dll.hoi4_backend_request.argtypes=[ctypes.c_uint64,ctypes.c_void_p,ctypes.c_size_t];dll.hoi4_backend_request.restype=ctypes.c_void_p
            dll.hoi4_backend_release.argtypes=[ctypes.c_uint64];dll.hoi4_backend_release.restype=ctypes.c_bool
            dll.hoi4_backend_free.argtypes=[ctypes.c_void_p];dll.hoi4_backend_free.restype=None
            info=_reply(dll,dll.hoi4_backend_info())
            if info.get('source_catalogs')!=1 or info.get('catalog_generation')!=1:
                raise ValueError('Outdated Rust backend: rebuild with scripts/build-backend.ps1')
        except (OSError,ValueError,AttributeError) as exc:raise BackendUnavailable('Rust backend could not be loaded: '+str(exc)) from exc
        _dll=dll;return dll


def _reply(dll,pointer):
    if not pointer:raise BackendUnavailable('Rust backend returned no result')
    try:value=json.loads(ctypes.string_at(pointer))
    finally:dll.hoi4_backend_free(pointer)
    if not isinstance(value,dict) or value.get('schema_version')!=1:raise BackendUnavailable('Invalid backend response')
    if 'error' in value:raise ValueError(value['error'])
    return value['result']


def _request(owner_handle,op,**args):
    data=json.dumps({'schema_version':1,'op':op,'args':args},ensure_ascii=False,separators=(',',':'),allow_nan=False).encode('utf8')
    if len(data)>(64 if op.startswith('identity_') else 32)*1024*1024:raise ValueError('Backend request exceeds limit')
    dll=_library();buffer=ctypes.create_string_buffer(data)
    return _reply(dll,dll.hoi4_backend_request(owner_handle,buffer,len(data)))


def _publish_in_place(target,source):
    """Preserve container references held by compatibility import callers."""
    if isinstance(target,dict) and isinstance(source,dict):
        for key in set(target)-source.keys():del target[key]
        for key,value in source.items():
            if key in target and isinstance(target[key],(dict,list)) and type(target[key]) is type(value):
                _publish_in_place(target[key],value)
            else:target[key]=value
    elif isinstance(target,list) and isinstance(source,list):
        for index,value in enumerate(source):
            if index>=len(target):target.append(value)
            elif isinstance(target[index],(dict,list)) and type(target[index]) is type(value):_publish_in_place(target[index],value)
            else:target[index]=value
        del target[len(source):]


def engine_info():
    dll=_library();return _reply(dll,dll.hoi4_backend_info())


def state_signature(value):return _request(0,'signature',value=value)


class NativeMonitor:
    def __init__(self):
        self._handle=_request(0,'monitor_open')
        self._finalizer=weakref.finalize(self,_request,0,'monitor_release',handle=self._handle)
    def accept(self,value,now,observed_at):return _request(0,'monitor_accept',handle=self._handle,value=value,now=now,observed_at=observed_at)
    def failed(self,error,now):return _request(0,'monitor_error',handle=self._handle,error=error,now=now)
    def close(self):self._finalizer()


class NativeEvents:
    def __init__(self,interval,maximum):
        self._handle=_request(0,'open',interval=interval,maximum=maximum)
        self._finalizer=weakref.finalize(self,_library().hoi4_backend_release,self._handle)
    def subscribe(self,now):return _request(self._handle,'subscribe',now=now)
    def unsubscribe(self,identity):return _request(self._handle,'unsubscribe',id=identity)
    def publish(self,value,now):return _request(self._handle,'publish',value=value,now=now)
    def heartbeat(self,now):return _request(self._handle,'heartbeat',now=now)
    def wait(self,identity,seconds,now):return _request(self._handle,'wait',id=identity,seconds=seconds,now=now)
    def peek(self,now):return _request(self._handle,'peek',now=now)
    def stop(self):return _request(self._handle,'close')
    def close(self):self._finalizer()
    def __enter__(self):return self
    def __exit__(self,*_):self.close()
