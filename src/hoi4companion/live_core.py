"""Qualified focus/research decoding through Rust, preserving witnessed reads.

Callbacks are read-only compatibility transport. They keep the existing process
handle, read budget, witnesses and RTTI source; no new process rights are opened.
"""
import ctypes
import json
from .backend_core import _library,_reply

_READ=ctypes.CFUNCTYPE(ctypes.c_int32,ctypes.c_void_p,ctypes.c_uint64,ctypes.c_void_p,ctypes.c_size_t)
_CLASS=_READ


def read_core(sample,*,watch=False):
    from .live import LiveReadError,WORLD_RVA,WindowsGame
    process=sample.process;profile=getattr(process,'profile',None);errors=[]
    request={'schema_version':1,'base':process.base,'world_rva':getattr(profile,'world_rva',WORLD_RVA),
        'definition_vtable':getattr(profile,'technology_definition_vtable',0x293C2E0),
        'joint_focus_vtable':getattr(profile,'joint_focus_vtable',None),'watch':watch}
    if isinstance(process,WindowsGame):return read_process_core(sample,request)
    @_READ
    def read(_context,address,output,size):
        try:
            value=sample.read(address,size)
            ctypes.memmove(output,value,size);return 0
        except Exception as exc:errors.append(exc);return -1
    @_CLASS
    def class_name(_context,rva,output,capacity):
        try:
            value=profile.class_name(rva).encode('ascii')
            if len(value)>capacity:raise LiveReadError('RTTI non riconosciuto')
            ctypes.memmove(output,value,len(value));return len(value)
        except Exception as exc:errors.append(exc);return -1
    data=json.dumps(request,allow_nan=False).encode('utf8');buffer=ctypes.create_string_buffer(data);dll=_library()
    try:
        function=dll.hoi4_backend_read_core
        function.argtypes=[ctypes.c_void_p,ctypes.c_size_t,_READ,_CLASS,ctypes.c_void_p];function.restype=ctypes.c_void_p
        result=_reply(dll,function(buffer,len(data),read,class_name,None))
    except ValueError as exc:
        if errors:raise errors[0] from exc
        raise LiveReadError(str(exc)) from exc
    if errors:raise errors[0]
    return result


def read_process_core(sample,request):
    """Borrow the already qualified host handle and immutable PE bytes once."""
    from .live import LiveReadError
    process=sample.process;dll=_library()
    data=json.dumps(request,allow_nan=False).encode('utf8');buffer=ctypes.create_string_buffer(data)
    try:
        if hasattr(process.profile,'_native_handle'):
            function=dll.hoi4_backend_read_profile_process
            function.argtypes=[ctypes.c_uint64,ctypes.c_void_p,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_size_t];function.restype=ctypes.c_void_p
            result=_reply(dll,function(process.profile._native_handle,process.handle,buffer,len(data),sample.budget-sample.used))
        else:
            pe=process.profile.data;pe_pointer=ctypes.c_char_p(pe)
            function=dll.hoi4_backend_read_process
            function.argtypes=[ctypes.c_void_p,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_size_t];function.restype=ctypes.c_void_p
            result=_reply(dll,function(process.handle,buffer,len(data),pe_pointer,len(pe),sample.budget-sample.used))
        used=result.pop('bytes_used');witnesses=result.pop('witnesses')
        if type(used) is not int or not 0<=used<=sample.budget-sample.used:raise ValueError('Invalid native read budget')
        sample.used+=used
        for address,size,text in witnesses:
            raw=bytes.fromhex(text)
            if len(raw)!=size:raise ValueError('Partial native witness')
            old=sample.witnesses.setdefault((address,size),raw)
            if old!=raw:raise LiveReadError('Stato cambiato durante la lettura')
        return result
    except ValueError as exc:raise LiveReadError(str(exc)) from exc
