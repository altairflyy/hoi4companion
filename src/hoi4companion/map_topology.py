"""Static native land topology, cached by installed map generation."""

def load_neighbors(game, mods):
    from .backend_core import _request
    value=_request(0,'source_neighbors',game=str(game),mods=list(map(str,mods)))
    return {int(p):neighbors for p,neighbors in value['neighbors'].items()}
