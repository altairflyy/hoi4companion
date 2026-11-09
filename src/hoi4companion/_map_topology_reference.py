"""Static land adjacency from the installed map, including map-mod overrides."""
import csv
from functools import lru_cache
from pathlib import Path
from PIL import Image
from .installation import collect_files


def load_neighbors(game, mods):
    files = {}
    for suffix in ('.bmp', '.csv'):
        paths, _ = collect_files(game, mods, 'map', suffix)
        files.update((Path(path).name, Path(path)) for path in paths)
    if not all(name in files for name in ('definition.csv', 'provinces.bmp')):
        return {}
    selected = [files.get(name) for name in ('definition.csv','provinces.bmp','adjacencies.csv')]
    key = tuple((str(p.resolve()), p.stat().st_size, p.stat().st_mtime_ns) if p else None for p in selected)
    return {p:list(neighbors) for p,neighbors in _load(key).items()}


@lru_cache(maxsize=2)
def _load(key):
    definitions = {}
    with open(key[0][0], encoding='utf-8-sig') as source:
        for row in csv.reader(source, delimiter=';'):
            if len(row) < 5 or not row[0].isdigit():
                continue
            identifier = int(row[0]); color = tuple(map(int, row[1:4]))
            if identifier <= 0 or any(not 0 <= n <= 255 for n in color):
                continue
            if row[4].lower() == 'land':
                code = color[0] << 16 | color[1] << 8 | color[2]
                if code in definitions:
                    raise ValueError('Duplicate land province color')
                definitions[code] = identifier
    graph = {identifier:set() for identifier in definitions.values()}
    with Image.open(key[1][0]) as image:
        if image.width * image.height > 100_000_000:
            raise ValueError('Province map exceeds topology limit')
        image = image.convert('RGB')
        try:
            import numpy as np
        except ImportError:
            previous = None; edges = set(); used = set()
            for y in range(image.height):
                row = [r << 16 | g << 8 | b for r,g,b in image.crop((0,y,image.width,y+1)).get_flattened_data()]
                used.update(row)
                for a,b in zip(row, row[1:] + row[:1]):
                    if a != b: edges.add((min(a,b),max(a,b)))
                if previous is not None:
                    for a,b in zip(row,previous):
                        if a != b: edges.add((min(a,b),max(a,b)))
                previous = row
        else:
            rgb = np.asarray(image, dtype=np.uint32)
            codes = rgb[:,:,0] << 16 | rgb[:,:,1] << 8 | rgb[:,:,2]
            used=set(np.unique(codes).tolist())
            edges = set()
            for a,b in ((codes[:,:-1],codes[:,1:]), (codes[:-1],codes[1:]), (codes[:,-1],codes[:,0])):
                mask = a != b
                pairs = np.stack((np.minimum(a[mask],b[mask]),np.maximum(a[mask],b[mask])),axis=1)
                edges.update(map(tuple,np.unique(pairs,axis=0).tolist()))
    # A definition absent from the actual bitmap is unknown topology, not an
    # observed isolated province. Do not silently erase its possible frontier.
    present={identifier for code,identifier in definitions.items() if code in used}
    graph={identifier:values for identifier,values in graph.items() if identifier in present}
    for a,b in edges:
        left,right = definitions.get(a), definitions.get(b)
        if left is not None and right is not None:
            graph[left].add(right); graph[right].add(left)
    if key[2]:
        with open(key[2][0],encoding='utf-8-sig') as source:
            for row in csv.reader(source,delimiter=';'):
                if len(row) >= 3 and row[0].isdigit() and row[1].isdigit() and row[2].lower() == 'impassable':
                    a,b = int(row[0]),int(row[1])
                    graph.get(a,set()).discard(b); graph.get(b,set()).discard(a)
    return {p:tuple(sorted(neighbors)) for p,neighbors in graph.items()}
