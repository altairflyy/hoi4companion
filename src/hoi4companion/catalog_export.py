"""Compact AI projection; the installed, localized resolver catalog is untouched."""
from copy import deepcopy
import re
from .tooltips import localise


def _values(labels):
    values={v for v in labels.values() if isinstance(v,str)}
    # Catalog builders expand the same static references before creating aliases.
    # Long descriptions cannot be action reference names.
    values.update(localise(v,labels) for v in tuple(values) if '$' in v and len(v)<=4096)
    return values


def for_ai(entries, reference=None, localized=None):
    """Export distinct aliases attested by the English localization dictionary.

    No language guessing or fuzzy admission. Unclassified aliases are retained
    only in the internal catalog, never in these AI attachments.
    """
    english=_values(reference or {})
    result=[]
    for entry in entries:
        row=deepcopy(entry)
        display=row.pop('localized_name',None)
        name=row.get('name','')
        same_name=' '.join(name.split()).casefold()
        def keep(alias):
            return (isinstance(alias,str) and bool(alias)
                    and ' '.join(alias.split()).casefold()!=same_name and alias in english)
        aliases=list(dict.fromkeys(a for a in row.pop('aliases',[]) if keep(a)))
        if aliases:row['aliases']=aliases
        if 'country_aliases' in row:
            country={k:list(dict.fromkeys(a for a in values if keep(a))) for k,values in row['country_aliases'].items()}
            country={k:v for k,v in country.items() if v}
            if country:row['country_aliases']=country
            else:row.pop('country_aliases')
        # A local-only mod may have no readable English reference at all.
        if (isinstance(display,str) and display and display!=name and name not in english
                and (name==row.get('id') or re.fullmatch(r'[A-Z0-9_]+',name))):
            row['fallback_name']=display
        result.append(row)
    return result
