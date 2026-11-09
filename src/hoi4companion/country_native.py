"""Native country decoders with the same witnessed sample/watch contracts."""
import ctypes
import json
from .backend_core import _library,_reply
from .live_core import _READ,_CLASS


def read_section(reader,section,**extra):
    from .live import WindowsGame,LiveReadError
    process=reader.process;sample=reader.s;errors=[]
    request={'schema_version':1,'base':process.base,'world':reader.world,'country':reader.country,
        'tag':reader.tag,'section':section,'attributes':{
            'building_database_rva':getattr(process.profile,'building_database_rva',0x332EE28),
            'production_owner_offset':getattr(process.profile,'production_owner_offset',0x290),
            'production_totals':getattr(process.profile,'production_totals',(0x2B8,0x318,0x378)),
            'sha256':getattr(process.profile,'sha256',''),
            'world_rva':getattr(process.profile,'world_rva',0),
            'campaign_signature':getattr(process.profile,'campaign_signature',bytes.fromhex('4c8d86280700004533c9ba803b0000488bcb')),
            'campaign_signature_rva':getattr(process.profile,'campaign_signature_rva',0x1F2F2D),
            'campaign_token_rva':getattr(process.profile,'campaign_token_rva',0x3423158),
            'military_layout':getattr(process.profile,'military_layout',None),
            'country_metrics_layout':getattr(process.profile,'country_metrics_layout',None),
            'foreign_interactions_layout':getattr(process.profile,'foreign_interactions_layout',None),
            'province_neighbors':getattr(process.profile,'province_neighbors',None) if section=='fortifications' else None,
            'event_cache':getattr(process,'_hoi4_event_cache',None) if section=='events' else None,
            'executable_bytes':callable(getattr(process.profile,'bytes',None)),**extra}}
    def serialise(value):
        if isinstance(value,bytes):return value.hex()
        if isinstance(value,dict):return {k:serialise(v) for k,v in value.items()}
        if isinstance(value,(list,tuple)):return [serialise(v) for v in value]
        return value
    data=json.dumps(serialise(request),allow_nan=False).encode('utf8');buffer=ctypes.create_string_buffer(data);dll=_library()
    try:
        if isinstance(process,WindowsGame):
            function=dll.hoi4_backend_country_profile_process
            function.argtypes=[ctypes.c_uint64,ctypes.c_void_p,ctypes.c_void_p,ctypes.c_size_t,ctypes.c_size_t];function.restype=ctypes.c_void_p
            result=_reply(dll,function(process.profile._native_handle,process.handle,buffer,len(data),sample.budget-sample.used))
            used=result.pop('bytes_used')
            if type(used) is not int or not 0<=used<=sample.budget-sample.used:raise ValueError('Invalid native read budget')
            sample.used+=used
            for address,size,text in result.pop('witnesses'):
                raw=bytes.fromhex(text)
                if len(raw)!=size:raise ValueError('Partial native country witness')
                if sample.witnesses.setdefault((address,size),raw)!=raw:raise LiveReadError('Stato cambiato durante la lettura')
        else:
            @_READ
            def read(_context,address,output,size):
                try:ctypes.memmove(output,sample.read(address,size),size);return 0
                except Exception as exc:errors.append(exc);return -len(errors)
            @_CLASS
            def class_name(_context,rva,output,capacity):
                try:
                    value=process.profile.class_name(rva).encode('ascii')
                    if len(value)>capacity:raise LiveReadError('RTTI non riconosciuto')
                    ctypes.memmove(output,value,len(value));return len(value)
                except Exception as exc:errors.append(exc);return -len(errors)
            @_READ
            def executable(_context,rva,output,size):
                try:
                    value=process.profile.bytes(rva,size)
                    if len(value)!=size:raise LiveReadError('Partial executable read')
                    ctypes.memmove(output,value,size);return 0
                except Exception as exc:errors.append(exc);return -len(errors)
            @_READ
            def error(_context,index,output,capacity):
                if index>=len(errors):return 0
                value=str(errors[index]).encode('utf8')[:capacity]
                ctypes.memmove(output,value,len(value));return len(value)
            @_READ
            def discovery(_context,address,output,size):
                try:
                    value=process.read(address,size)
                    if len(value)!=size:raise LiveReadError('Partial GUI discovery read')
                    ctypes.memmove(output,value,size);return 0
                except Exception as exc:errors.append(exc);return -len(errors)
            function=dll.hoi4_backend_country_v3
            function.argtypes=[ctypes.c_void_p,ctypes.c_size_t,_READ,_CLASS,_READ,_READ,_READ,ctypes.c_void_p];function.restype=ctypes.c_void_p
            result=_reply(dll,function(buffer,len(data),read,class_name,executable,error,discovery,None))
    except ValueError as exc:
        raise LiveReadError(str(exc)) from exc
    for address,size,text in result['watch']:
        raw=bytes.fromhex(text)
        if len(raw)!=size or sample.witnesses.get((address,size))!=raw:raise LiveReadError('Campione incoerente: riprovare')
        reader.watch[address,size]=raw
    if 'event_cache' in result:
        root,owners,headers=result['event_cache']
        process._hoi4_event_cache=(root,owners,tuple(bytes.fromhex(v) for v in headers))
    return result['value']
