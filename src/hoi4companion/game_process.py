"""Compatibility proxy for native-owned read-only process handles."""
import ctypes
import os
import weakref
from .backend_core import _request,_library,_reply


def release(handle):_request(0,'game_release',handle=handle)


class NativeGame:
    def __init__(self,profile):
        from .live import LiveReadError
        if os.name!='nt':raise LiveReadError('Lettura live disponibile solo su Windows')
        try:result=_request(0,'game_open',profile=profile._native_handle)
        except ValueError as exc:raise LiveReadError(str(exc)) from exc
        self.profile=profile;self._native_handle=result['handle']
        self._finalizer=weakref.finalize(self,release,self._native_handle)
        info=result['info'];self.pid,self.base,self.created,self.handle=(info[k] for k in ('pid','base','created','handle'))

    def identity(self):
        from .live import LiveReadError
        try:return _request(0,'game_identity',handle=self._native_handle)
        except ValueError as exc:raise LiveReadError(str(exc)) from exc

    def read(self,address,size):
        from .live import LiveReadError
        try:return bytes.fromhex(_request(0,'game_read',handle=self._native_handle,address=address,size=size))
        except ValueError as exc:raise LiveReadError(str(exc)) from exc

    def close(self):
        if hasattr(self,'_finalizer'):self._finalizer()
        self.handle=None

    def _compare(self,packet):
        from .live import LiveReadError
        dll=_library();function=dll.hoi4_backend_game_compare
        function.argtypes=[ctypes.c_uint64,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_uint32];function.restype=ctypes.c_void_p
        buffer=ctypes.create_string_buffer(packet)
        try:return _reply(dll,function(self._native_handle,buffer,len(packet),2))
        except ValueError as exc:raise LiveReadError(str(exc)) from exc

    def verify_sample(self,witnesses,visibility=None):
        from .live import LiveReadError
        from .native_observer import encode_sample
        if self._compare(encode_sample(witnesses,visibility)):raise LiveReadError('Campione incoerente: riprovare')
        return True

    def prepare_watch(self,reads):
        from .native_observer import encode_watch
        return encode_watch(reads)

    def probe_watch(self,packet):return self._compare(packet)
