"""Compatibility access to one immutable native static-geography index."""
from collections import defaultdict
import weakref
from .backend_core import _request


class GeographyIndex:
    def __init__(self,catalog):
        args=catalog.native_import_args() if hasattr(catalog,'native_import_args') else {
            'entries':[row for row in catalog if row.get('kind')=='state']}
        self._handle=_request(0,'geography_open',**args)['handle']
        self._release=weakref.finalize(self,_request,0,'geography_release',handle=self._handle)
        data=_request(0,'geography_export',handle=self._handle)
        self.states=defaultdict(list,{int(key):rows for key,rows in data['states'].items()})
        self.names=defaultdict(set,{key:{tuple(pair) for pair in pairs} for key,pairs in data['names'].items()})
        self.provinces=defaultdict(set,{int(key):set(values) for key,values in data['provinces'].items()})
        self.neighbors={int(key):values for key,values in data['neighbors'].items()}

    def resolve(self,params,provincial=False):
        return _request(0,'geography_resolve',handle=self._handle,params=params,provincial=provincial)
