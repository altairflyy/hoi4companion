"""Compatibility transport to the Rust static analyzer and pinned CWTools rules."""
from pathlib import Path
from functools import lru_cache
from .backend_core import _request
from .script_document import encode
RULE_ROOT=Path(__file__).with_name('rules')/'cwtools'

class RuleBook:
    def __init__(self,root):
        self._compiled=_request(0,'script_rulebook',root=str(root))
        for field in ('provenance','rules','links','load_errors'):setattr(self,field,self._compiled[field])
        self.enums={key:set(values) for key,values in self._compiled['enums'].items()}
    @classmethod
    @lru_cache(maxsize=1)
    def load(cls):return cls(RULE_ROOT)
    def describe(self,kind,name):return _request(0,'script_rule_describe',book=self._compiled,kind=kind,name=name)

def analyze_documents(documents,labels=None,reference_labels=None,*,book=None,incomplete=(),limit=2000):
    if type(limit) is not int or not 1<=limit<=10000:raise ValueError('Limite diagnosi non valido')
    selected=book or RuleBook.load()
    payload={'provenance':selected.provenance,'rules':selected.rules,'links':selected.links,
             'enums':{key:sorted(values)for key,values in selected.enums.items()},'load_errors':selected.load_errors}
    return _request(0,'script_analyze_documents',documents=[[file,encode(node)] for file,node in documents],labels=labels or {},reference=reference_labels or {},book=payload,incomplete=list(incomplete),limit=limit)

def analyze_installation(game,mods,labels,reference_labels):
    from .cwtools_engine import analyze_installation
    return analyze_installation(game,mods)
