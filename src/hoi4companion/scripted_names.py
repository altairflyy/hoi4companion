"""Bounded identity aliases from scripted localisation; no trigger evaluation."""
import re
from .installation import collect_files
from .jomini_parser import parse_file

TOKEN=re.compile(r'\[(?:(?P<scope>[A-Za-z_][A-Za-z0-9_]*|[0-9]+)\.)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\]')


def load_names(game,mods):
    files,warnings=collect_files(game,mods,'common/scripted_localisation','.txt')
    definitions={}
    for path in files:
        try:
            for node in parse_file(path).blocks('defined_text'):
                name=node.scalar('name')
                if name:
                    definitions[name]=list(dict.fromkeys(t.scalar('localization_key') for t in node.blocks('text') if t.scalar('localization_key')))
        except (OSError,ValueError,RecursionError) as exc:
            warnings.append(f'Scripted names ({path.name}): {exc}')
    return definitions,warnings


class NameExpander:
    """Generation-owned adjective index, shared by a complete catalog pass."""
    def __init__(self,definitions,labels):
        self.definitions,self.labels=definitions,labels
        self.adjectives={}
        for key,value in labels.items():
            match=re.fullmatch(r'([A-Z0-9]{3})(?:_.*)?_ADJ',key)
            if match and isinstance(value,str) and 0<len(value)<=128 and '[' not in value and ']' not in value:
                self.adjectives.setdefault(match[1],set()).add(value)

    def expand(self,text,*,countries=()):
        return expand_names(text,self.definitions,self.labels,countries=countries,adjectives=self.adjectives)


def expand_names(text,definitions,labels,*,countries=(),adjectives=None):
    """Return possible identity labels, never the live branch or country state.

    A country-scoped scripted function shares its declared alternative labels.
    Built-in adjective aliases use explicit tags (or the caller's eligible root
    tags); numeric state references use the exact STATE_n key. Other runtime
    scopes, leaders, capitals and dynamic country names deliberately stay unknown.
    """
    def alternatives(match):
        scope,name=match['scope'],match['name']
        if scope and scope.casefold()!='root' and not re.fullmatch(r'[A-Z0-9]{3}|[0-9]+',scope):
            return []
        if scope and name=='GetAdjective':
            tags=countries if scope.casefold()=='root' else [scope]
            if adjectives is not None:
                return sorted({value for tag in tags if isinstance(tag,str) and re.fullmatch(r'[A-Z0-9]{3}',tag)
                               for value in adjectives.get(tag,())})[:64]
            prefixes=[re.compile(re.escape(tag)+r'(?:_.*)?_ADJ\Z')
                      for tag in tags if isinstance(tag,str) and re.fullmatch(r'[A-Z0-9]{3}',tag)]
            return sorted({value for key,value in labels.items()
                           if any(pattern.fullmatch(key) for pattern in prefixes)
                           and isinstance(value,str) and 0<len(value)<=128
                           and '[' not in value and ']' not in value})[:64]
        if scope and scope.isdecimal():
            label=labels.get('STATE_'+scope) if name=='GetName' else None
            return [label] if isinstance(label,str) else []
        return [labels[key] for key in definitions.get(name,[])[:64]
                if isinstance(labels.get(key),str)]

    def expand(value,seen,depth):
        if depth>8 or len(value)>4096:return []
        match=TOKEN.search(value)
        if not match:return [value] if '[' not in value and ']' not in value else []
        name=match['name']
        if name in seen:return []
        result=set()
        for label in alternatives(match):
            for expanded in expand(label,seen|{name},depth+1):
                for item in expand(value[:match.start()]+expanded+value[match.end():],seen,depth+1):
                    result.add(item)
                    if len(result)>=64:return sorted(result)
        return sorted(result)
    return expand(text,set(),0) if isinstance(text,str) and TOKEN.search(text) else []
