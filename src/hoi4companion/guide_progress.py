"""Rust-owned run/campaign transitions and qualified evidence observations."""
from .progress_core import NativeRuns


class RunStore(NativeRuns):
    @property
    def _runs(self):
        # Detached compatibility view for diagnostic/test fixtures only.
        return self._call('snapshot')['runs']

    def observe(self,guide,live_state,*,allow_research=True):
        from .backend_core import _request
        with self._lock:
            if self._owner is not None:
                from .progress_storage import ProgressStorageError
                try:return _request(0,'runs_observe',handle=self._owner,guide=guide,state=live_state,allow_research=allow_research)
                except ValueError as exc:
                    if self._database:raise ProgressStorageError(str(exc)) from exc
                    raise
            record=self.get(guide['id'])
            runs=self._call('snapshot')['runs']
            result=_request(0,'guide_observe',guide=guide,state=live_state,record=record,
                            runs=runs,allow_research=allow_research)
            if not result['changed']:return False
            return self._call('observe_replace',records={record['run_id']:result['record']},value=True)
