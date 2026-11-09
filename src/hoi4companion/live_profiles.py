"""Shared build-pinned profile data and native immutable PE generations."""
import hashlib
import json
from pathlib import Path
import weakref
from .backend_core import _request

MANIFEST=Path(__file__).with_name('live-profiles-v1.json')


def decode(value):
    if isinstance(value,dict):
        if set(value)=={'$bytes'}:return bytes.fromhex(value['$bytes'])
        if set(value)=={'$tuple'}:return tuple(decode(v) for v in value['$tuple'])
        return {k:decode(v) for k,v in value.items()}
    if isinstance(value,list):return [decode(v) for v in value]
    return value


def profiles():return decode(json.loads(MANIFEST.read_bytes())['profiles'])


def release(handle):_request(0,'profile_release',handle=handle)


class NativeProfile:
    def __init__(self,path):
        from .live import LiveReadError
        try:
            result=_request(0,'profile_open',path=str(Path(path).resolve()),manifest_sha256=hashlib.sha256(MANIFEST.read_bytes()).hexdigest())
        except ValueError as exc:raise LiveReadError(str(exc)) from exc
        self._native_handle=result['handle'];self._finalizer=weakref.finalize(self,release,self._native_handle)
        metadata=result['metadata'];self.path=Path(metadata['path']).resolve()
        for key in ('sha256','image_base','image_size','timestamp'):setattr(self,key,metadata[key])
        self.sections=[tuple(v) for v in metadata['sections']]
        for key,value in decode(metadata['selected']).items():setattr(self,key,value)

    def bytes(self,rva,size):
        from .live import LiveReadError
        try:return bytes.fromhex(_request(0,'profile_bytes',handle=self._native_handle,rva=rva,size=size))
        except ValueError as exc:raise LiveReadError(str(exc)) from exc

    def class_name(self,rva):
        from .live import LiveReadError
        try:return _request(0,'profile_class',handle=self._native_handle,rva=rva)
        except ValueError as exc:raise LiveReadError(str(exc)) from exc

    def close(self):self._finalizer()
