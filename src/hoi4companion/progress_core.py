"""Compatibility adapter for Rust-owned run/campaign state and transactions."""
import ctypes
import json
from threading import RLock
from uuid import uuid4
import weakref
from .storage_core import _library,_reply,StorageCoreUnavailable


class NativeRuns:
    def __init__(self,id_factory=None,*,path=None):
        self._owner=None
        if id_factory is None:
            from .backend_core import _request
            from .progress_storage import ProgressStorageError
            self._lock=RLock()
            try:self._owner=_request(0,'runs_open',path=str(path) if path is not None else None)
            except ValueError as exc:raise ProgressStorageError(str(exc)) from exc
            self._finalizer=weakref.finalize(self,_request,0,'runs_release',handle=self._owner)
            self._database=_OwnedDatabase(self._owner,path) if path is not None else None
            return
        # Explicit deterministic ID/I/O injection is a fixture transport. The
        # application path above keeps the complete transaction in Rust.
        self._lock=RLock();self._id_factory=id_factory or (lambda:uuid4().hex)
        self._database=None;self._dll=_library()
        try:
            self._dll.hoi4_progress_open.argtypes=[];self._dll.hoi4_progress_open.restype=ctypes.c_void_p
            self._dll.hoi4_progress_request.argtypes=[ctypes.c_uint64,ctypes.c_void_p,ctypes.c_size_t];self._dll.hoi4_progress_request.restype=ctypes.c_void_p
            self._dll.hoi4_progress_release.argtypes=[ctypes.c_uint64];self._dll.hoi4_progress_release.restype=ctypes.c_bool
        except AttributeError as exc:raise StorageCoreUnavailable('Rebuild Rust storage: campaign transitions missing') from exc
        self._handle=_reply(self._dll,self._dll.hoi4_progress_open())['handle']
        self._finalizer=weakref.finalize(self,self._dll.hoi4_progress_release,self._handle)
        if path is not None:
            from .progress_storage import ProgressDatabase
            try:
                self._database=ProgressDatabase(path);value=self._database.load()
                if value is not None:self._load(value)
            except Exception:self.close();raise

    def _request(self,op,args=None,ids=()):
        data=json.dumps({'schema_version':1,'op':op,'args':args or {},'ids':list(ids)},ensure_ascii=False,allow_nan=False).encode('utf8')
        if len(data)>96*1024*1024:raise ValueError('Progress request exceeds limit')
        buffer=ctypes.create_string_buffer(data)
        return _reply(self._dll,self._dll.hoi4_progress_request(self._handle,buffer,len(data)))

    def _call(self,op,**args):
        with self._lock:
            if self._owner is not None:
                from .backend_core import _request
                from .progress_storage import ProgressStorageError
                try:return _request(0,'runs_call',handle=self._owner,operation=op,args=args)
                except ValueError as exc:
                    if self._database:raise ProgressStorageError(str(exc)) from exc
                    raise
            ids=[]
            while True:
                result=self._request(op,args,ids)
                if not result.get('needs_id'):break
                if len(ids)>=20000:raise ValueError('Progress ID limit exceeded')
                ids.append(self._id_factory())
            if result['changed']:
                try:
                    if self._database:self._database.save(result['snapshot'])
                except Exception:
                    self._request('rollback');raise
                self._request('accept')
            return result['value']

    def ensure(self,guide_id,legacy_checks=None):
        if isinstance(legacy_checks,dict):legacy_checks={k:v for k,v in legacy_checks.items() if isinstance(k,str) and type(v) is bool}
        return self._call('ensure',guide_id=guide_id,checks=legacy_checks)
    def create(self,guide_id,session_id=None):return self._call('create',guide_id=guide_id,session_id=session_id)
    def select(self,guide_id,run_id):return self._call('select',guide_id=guide_id,run_id=run_id)
    def get(self,guide_id,run_id=None):return self._call('get',guide_id=guide_id,run_id=run_id)
    def update(self,guide_id,checks,*,started=None):
        for value in (checks,started) if started is not None else (checks,):
            if not isinstance(value,dict) or any(not isinstance(k,str) or type(v) is not bool for k,v in value.items()):raise ValueError('Conferme della partita non valide')
        return self._call('update',guide_id=guide_id,checks=checks,started=started)
    def bind(self,session_id):return self._call('bind',session_id=session_id)
    def bind_campaign(self,campaign_id,country,session_id,tick):return self._call('bind_campaign',campaign_id=campaign_id,country=country,session_id=session_id,tick=tick)
    def history(self,guide_id):return self._call('history',guide_id=guide_id)
    def metadata(self):return self._call('metadata')
    def snapshot(self):return self._call('snapshot')
    def _load(self,snapshot):return self._call('load',snapshot=snapshot)
    def close(self):
        if hasattr(self,'_finalizer'):self._finalizer()
    def __enter__(self):return self
    def __exit__(self,*_):self.close()


class _OwnedDatabase:
    """Diagnostic view; no Python save or transaction is used at runtime."""
    def __init__(self,owner,path):self.owner=owner;self.path=path
    @property
    def revision(self):
        from .backend_core import _request
        return _request(0,'runs_status',handle=self.owner)['revision']
