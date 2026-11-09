"""Conservative explanatory hints, never implicit game actions or conditions."""
import re
from functools import lru_cache
from .research_upgrades import guide_language


LAWS = {name.casefold(): name for name in (
    'Civilian Economy', 'Early Mobilization', 'Partial Mobilization', 'War Economy', 'Total Mobilization',
    'Disarmed Nation', 'Volunteer Only', 'Limited Conscription', 'Extensive Conscription',
    'Service by Requirement', 'All Adults Serve', 'Scraping the Barrel',
    'Free Trade', 'Export Focus', 'Limited Exports', 'Closed Economy')}


@lru_cache(maxsize=1)
def _glossary():
    entries = {}
    for domain, codes in guide_language()['abbreviations'].items():
        for code, meaning in codes.items():
            if len(code) < 2:
                continue  # Single-letter design codes require template context.
            for spelling in {code, code.upper()}:
                entries.setdefault(spelling, []).append({'code': code, 'meaning': meaning, 'domain': domain})
    pattern = re.compile(r'(?<![A-Za-z0-9])(?:' + '|'.join(re.escape(k) for k in sorted(entries, key=len, reverse=True)) + r')(?![A-Za-z0-9])')
    return pattern, entries


def explanatory_terms(text, category=None):
    """Reading aids only. Ambiguous domains never become guessed actions."""
    if not isinstance(text, str):
        return []
    pattern, entries = _glossary()
    domain = {'construction': 'construction', 'production': 'production'}.get(category)
    result = []; seen = set()
    for match in pattern.finditer(text):
        candidates = entries[match[0]]
        scoped = [entry for entry in candidates if entry['domain'] == domain] if domain else []
        candidates = scoped or candidates
        if len({entry['meaning'] for entry in candidates}) != 1:
            continue
        entry = candidates[0]
        if entry['code'] in seen:
            continue
        seen.add(entry['code']);result.append(entry.copy())
        if len(result) == 24:
            break
    return result


def explicit_action(text):
    """Convert only a complete standalone instruction with explicit meaning."""
    if not isinstance(text,str):return None
    value=re.sub(r'^\s*-\s*','',text).strip()
    law=re.fullmatch(r'(.+?)(?:\s*\((\d+)\s*PP\))?\s*',value,re.I)
    if law and law[1].strip().casefold() in LAWS:
        result={'type':'law','target':LAWS[law[1].strip().casefold()]}
        if law[2] is not None:result['political_power_cost']=int(law[2])
        return result
    if re.fullmatch(r'create\s+(?:an?\s+)?(?:intelligence\s+)?agency',value,re.I):
        return {'type':'create_agency'}
    build=re.fullmatch(r'build\s+(\d+)\s+(military|civilian)\s+factor(?:y|ies)\s+in\s+([^,:;()]+)',value,re.I)
    if build and 0<int(build[1])<=1000 and not re.search(r'\b(?:and|then|or|when|after)\b',build[3],re.I):
        return {'type':'build','building':build[2].lower()+' factory','state':build[3].strip(),
                'quantity':{'mode':'add','value':int(build[1])}}
    event=re.fullmatch(r'event\s*:\s*([^;\n]+?)\s*;\s*(?:choose|select)\s*:\s*([^;\n]+?)\s*',value,re.I)
    if event:return {'type':'event','event':event[1].strip(),'option':event[2].strip()}
    return None


def abbreviation_hint(text):
    if not isinstance(text,str):
        return None
    # An abbreviation such as MIL has different meanings across domains. Do
    # not choose a domain when both or neither are explicitly mentioned.
    build=bool(re.search(r'\b(?:build|construct|construction)\b',text,re.I))
    production=bool(re.search(r'\b(?:produce|production|producing|assign(?:ment|ed)?)\b',text,re.I))
    if build == production:
        return None
    domain='construction' if build else 'production'
    entries=guide_language()['abbreviations'][domain]
    found=[]
    for code,meaning in entries.items():
        if re.search(r'(?<![A-Za-z0-9])'+re.escape(code)+r's?(?![A-Za-z0-9])',text,re.I):
            found.append(code+' = '+meaning)
    return 'Abbreviations ('+domain+'): '+ '; '.join(found) if found else None
