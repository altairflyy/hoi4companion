"""Read-only scenario research context; never a live or saved campaign observation."""

def read_research_start(game,mods,country,dlcs,scripts,start_date=None):
    from .backend_core import _request
    from .script_document import encode
    inventory=None if dlcs is None else {'items':dlcs.items,'complete':dlcs.complete}
    return _request(0,'source_research_start',game=str(game),mods=list(map(str,mods)),country=country,
                    dlcs=inventory,scripts={key:encode(value) for key,value in scripts.items()},start_date=start_date)
