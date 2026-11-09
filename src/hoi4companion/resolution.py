"""Static catalog adapter. Rust owns matching, context and review decisions."""
from copy import deepcopy
from dataclasses import dataclass
import os
from threading import RLock

from .catalog_core import NativeCatalog


@dataclass(frozen=True)
class ResolutionContext:
    country: str | None = None
    scope: str | None = None


def optional_focus_name(query):
    if not isinstance(query,str):return None
    parts=[x.strip() for x in query.split('/')]
    if len(parts)!=2:return None
    empty={'nothing','none','no focus'}
    if parts[1].casefold() in empty:return parts[0] or None
    if parts[0].casefold() in empty:return parts[1] or None
    return None


class CatalogIndex:
    def __init__(self,entries,*,country_names=None,adjectives=(),reference_designs=None):
        self.reference_designs=deepcopy(reference_designs or {})
        self._entries=tuple(deepcopy(list(entries)))
        adjectives=tuple(adjectives)
        self._national_metadata={'country_names':deepcopy(country_names or {}),'adjectives':adjectives}
        self._native=NativeCatalog(self._entries,country_names=country_names,adjectives=adjectives)
        self._geography=None;self._lock=RLock();self._reference=None
        if os.environ.get('HOI4_CATALOG_CORE_VERIFY')=='1':
            from ._resolution_reference import CatalogIndex as Reference
            self._reference=Reference(self._entries,country_names=country_names,adjectives=adjectives,reference_designs=reference_designs)

    def _verify(self,method,result,*args,**kwargs):
        if self._reference is not None and result!=getattr(self._reference,method)(*args,**kwargs):
            raise ValueError('Native catalog comparison failed: '+method)
        return result

    @staticmethod
    def _context(context):
        context=context or ResolutionContext()
        return {'country':context.country,'scope':context.scope}

    def __iter__(self):return iter(self._entries)
    def __len__(self):return len(self._entries)
    def export(self):return deepcopy(list(self._entries))

    def native_import_args(self):
        """Borrow this immutable backend generation without serializing it again."""
        return {'identity_handle':self._native._handle}

    def diagnostics(self):
        return {'engine':'rust-catalog-core',**self._native.query('stats')}

    def with_entries(self,entries):
        """A new immutable generation with the same national naming context."""
        return CatalogIndex(entries,**self._national_metadata,reference_designs=self.reference_designs)

    def resolve_country(self,query):
        if not isinstance(query,str) or not query.strip():return None
        return self._verify('resolve_country',self._native.query('resolve_country',query=query),query)

    def country_hint(self,title,guide):
        if not isinstance(title,str):return None
        return self._verify('country_hint',self._native.query('country_hint',query=title,guide=guide),title,guide)

    @property
    def geography(self):
        with self._lock:
            if self._geography is None:
                from .geography import GeographyIndex
                self._geography=GeographyIndex(self)
            return self._geography

    def rows(self,kinds,context=None,**filters):
        result=[self._entries[i] for i in self._native.query('rows',kinds=list(kinds),**self._context(context),filters=filters)]
        return self._verify('rows',result,kinds,context,**filters)

    def matches(self,query,kinds,context=None,*,explicit=False,typos=False,**filters):
        if not isinstance(query,str) or not query:return []
        result=[self._entries[i] for i in self._native.query('matches',query=query,kinds=list(kinds),**self._context(context),explicit=explicit,typos=typos,filters=filters)]
        return self._verify('matches',result,query,kinds,context,explicit=explicit,typos=typos,**filters)

    def excluded(self,query,kinds,context,**filters):
        if not isinstance(query,str) or not query:return []
        result=[{'entity':deepcopy(self._entries[row['index']]),'reason':row['reason']} for row in self._native.query('excluded',query=query,kinds=list(kinds),**self._context(context),filters=filters)]
        return self._verify('excluded',result,query,kinds,context,**filters)

    def focus_word_candidates(self,query,context):
        if not isinstance(query,str):return []
        result=[(self._entries[i],label) for i,label in self._native.query('focus_words',query=query,kinds=['focus'],**self._context(context))]
        return self._verify('focus_word_candidates',result,query,context)

    def suggest(self,query,kinds,context=None,*,limit=6):
        if not isinstance(query,str) or not query.strip():return []
        result=[{**deepcopy(self._entries[row['index']]),'suggestion':row['suggestion']} for row in self._native.query('suggest',query=query,kinds=list(kinds),**self._context(context),limit=limit)]
        return self._verify('suggest',result,query,kinds,context,limit=limit)

    def audit(self,query,kinds,context,*,explicit=False,**filters):
        result=self._native.query('audit',query=query,kinds=list(kinds),**self._context(context),explicit=explicit,filters=filters)
        return self._verify('audit',result,query,kinds,context,explicit=explicit,**filters)


def catalog_index(entries):
    return entries if isinstance(entries,CatalogIndex) else CatalogIndex(entries)
