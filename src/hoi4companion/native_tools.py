"""Rust-owned tool lifetimes. Python supplies arguments and consumes output only."""
import ctypes
from importlib.machinery import EXTENSION_SUFFIXES
import json
import math
import os
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
from threading import RLock
import time
import weakref

_dll=None
_lock=RLock()


class NativeToolsUnavailable(ValueError):
    pass


def _library():
    global _dll
    with _lock:
        if _dll is not None:return _dll
        folder=Path(__file__).parent/'_native';override=os.environ.get('HOI4_TOOLS_LIBRARY')
        suffix='.dll' if sys.platform=='win32' else '.dylib' if sys.platform=='darwin' else '.so'
        paths=[Path(override)] if override else [folder/('hoi4companion_tools'+s) for s in dict.fromkeys([suffix,*EXTENSION_SUFFIXES])]
        path=next((p for p in paths if p.is_file()),None)
        if path is None:raise NativeToolsUnavailable('Rust tool supervisor is not installed. Run scripts/build-tools.ps1.')
        try:
            dll=ctypes.CDLL(str(path.resolve()))
            dll.hoi4_tools_abi_version.argtypes=[];dll.hoi4_tools_abi_version.restype=ctypes.c_uint32
            if dll.hoi4_tools_abi_version()!=1:raise ValueError('Unsupported tool supervisor ABI')
            for name,args in [('hoi4_tools_info',[]),('hoi4_tools_spawn',[ctypes.c_void_p,ctypes.c_size_t]),
                ('hoi4_tools_status',[ctypes.c_uint64]),('hoi4_tools_wait',[ctypes.c_uint64,ctypes.c_uint32]),
                ('hoi4_tools_cancel',[ctypes.c_uint64]),('hoi4_tools_release',[ctypes.c_uint64])]:
                fn=getattr(dll,name);fn.argtypes=args;fn.restype=ctypes.c_void_p
            dll.hoi4_tools_free.argtypes=[ctypes.c_void_p];dll.hoi4_tools_free.restype=None
        except (OSError,ValueError,AttributeError) as exc:raise NativeToolsUnavailable('Rust tool supervisor could not be loaded: '+str(exc)) from exc
        _dll=dll
        return dll


def _reply(dll,ptr):
    if not ptr:raise NativeToolsUnavailable('Rust tool supervisor returned no result')
    try:value=json.loads(ctypes.string_at(ptr))
    finally:dll.hoi4_tools_free(ptr)
    if value.get('schema_version')!=1:raise NativeToolsUnavailable('Invalid tool supervisor response')
    if 'error' in value:raise ValueError('Rust tool supervisor: '+str(value['error']))
    return value['result']


def _release(dll,handle):
    _reply(dll,dll.hoi4_tools_release(handle))


def engine_info():
    dll=_library();return _reply(dll,dll.hoi4_tools_info())


class ManagedTool:
    def __init__(self,args,*,stdout=None,stderr=None,cwd=None,max_seconds=60,max_output_bytes=16*1024*1024):
        if os.name!='nt':raise NativeToolsUnavailable('Windows job objects are required')
        if not isinstance(args,(list,tuple)) or not args:raise ValueError('Tool arguments must be a non-empty vector')
        if not isinstance(max_seconds,(int,float)) or isinstance(max_seconds,bool) or not math.isfinite(max_seconds) or not 0<max_seconds<=86400:raise ValueError('Invalid tool deadline')
        self.args=list(map(str,args));self._lock=RLock();self._dll=_library();self._handle=None
        self._temporary=TemporaryDirectory(prefix='hoi4-tool-');root=Path(self._temporary.name)
        self._stdout=root/'stdout.txt' if stdout is None else Path(stdout.name).resolve()
        self._stderr=root/'stderr.txt' if stderr is None else Path(stderr.name).resolve()
        for file in (stdout,stderr):
            if file is not None:file.flush()
        spec={'schema_version':1,'argv':self.args,'stdout':str(self._stdout),'stderr':str(self._stderr),
              'cwd':str(cwd) if cwd else None,'max_millis':max(1,int(max_seconds*1000)),'max_output_bytes':max_output_bytes}
        try:
            data=json.dumps(spec,ensure_ascii=False,allow_nan=False).encode('utf8');buffer=ctypes.create_string_buffer(data)
            result=_reply(self._dll,self._dll.hoi4_tools_spawn(buffer,len(data)))
            self._handle=result['handle'];self._last=result['status'];self.pid=self._last['pid']
            self._finalizer=weakref.finalize(self,_release,self._dll,self._handle)
        except Exception:
            self._temporary.cleanup();raise

    @property
    def returncode(self):return self.status()['returncode']

    def status(self):
        with self._lock:
            if self._handle is not None:self._last=_reply(self._dll,self._dll.hoi4_tools_status(self._handle))
            return dict(self._last)

    def poll(self):return self.returncode

    def wait(self,timeout=None):
        if timeout is not None and (not isinstance(timeout,(int,float)) or not math.isfinite(timeout) or timeout<0):raise ValueError('Invalid wait timeout')
        deadline=time.monotonic()+timeout if timeout is not None else None
        while self.poll() is None:
            millis=1000 if deadline is None else max(0,min(1000,int((deadline-time.monotonic())*1000)))
            if deadline is not None and millis==0:raise subprocess.TimeoutExpired(self.args,timeout)
            with self._lock:
                if self._handle is not None:self._last=_reply(self._dll,self._dll.hoi4_tools_wait(self._handle,millis))
        return self.returncode

    def kill(self):
        with self._lock:
            if self._handle is not None:self._last=_reply(self._dll,self._dll.hoi4_tools_cancel(self._handle))

    terminate=kill

    def communicate(self,timeout=None):
        self.wait(timeout)
        def read(path):
            with path.open('rb') as stream:data=stream.read(64*1024*1024+1)
            if len(data)>64*1024*1024:raise ValueError('Tool output exceeds read limit')
            return data.decode('utf8',errors='replace').replace('\r\n','\n')
        return read(self._stdout),read(self._stderr)

    def close(self):
        with self._lock:
            if self._handle is None:return
            self.kill();self._finalizer();self._handle=None
            self._temporary.cleanup()

    def __enter__(self):return self
    def __exit__(self,*args):self.close()


def run(args,*,timeout=60,stdout=None,stderr=None,max_output_bytes=16*1024*1024):
    with ManagedTool(args,stdout=stdout,stderr=stderr,max_seconds=timeout,max_output_bytes=max_output_bytes) as process:
        out,err=process.communicate(timeout=timeout+2) if stdout is None and stderr is None else (None,None)
        if out is None:process.wait(timeout=timeout+2)
        status=process.status()
        if status['reason']=='timeout':raise subprocess.TimeoutExpired(list(args),timeout,output=out,stderr=err)
        if status['reason']=='output_limit':raise ValueError('Tool output exceeds limit')
        return subprocess.CompletedProcess(list(args),status['returncode'],out,err)
