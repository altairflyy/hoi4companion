"""Direct, audited weight-space edits. No loss, optimizer, gradient or training loop.

This is an experimental tool, not itself a natural-language interpreter.
It lets a reasoning agent inspect named tensors, propose an explicit sparse or
rank-one edit, and measure a candidate model against a fixed benchmark.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Mapping
import numpy as np

OPS = frozenset(('sparse_add', 'rank_one_add'))

def _finite(value, label):
    if isinstance(value, bool) or not isinstance(value, (float, int)) or not math.isfinite(value):
        raise ValueError(f'{label} must be a finite number')
    return float(value)

def _indices(value, size, label):
    if not isinstance(value, list) or not value:
        raise ValueError(f'{label}: expected nonempty list')
    if any(isinstance(x, bool) or not isinstance(x, int) or x < 0 or x >= size for x in value):
        raise ValueError(f'{label}: out of range')
    if len(set(value)) != len(value):
        raise ValueError(f'{label}: duplicate indices')
    return value

def validate_edit(tensors: Mapping[str, np.ndarray], patch: dict) -> None:
    if not isinstance(patch, dict) or patch.get('schema_version') != 1:
        raise ValueError('unsupported weight-edit schema')
    edits = patch.get('edits')
    if not isinstance(edits, list) or len(edits) > 256:
        raise ValueError('need at most 256 direct weight edits')
    fraction = _finite(patch.get('max_changed_fraction', .001), 'max_changed_fraction')
    if not (0 < fraction <= 1.):
        raise ValueError('invalid maximum edited fraction')
    for edit in edits:
        if not isinstance(edit, dict) or edit.get('op') not in OPS:
            raise ValueError('unsupported direct weight operation')
        name = edit.get('tensor')
        if not isinstance(name, str) or name not in tensors:
            raise ValueError(f'unknown tensor {name!r}')
        tensor = tensors[name]
        if tensor.ndim != 2 or tensor.size == 0 or not np.issubdtype(tensor.dtype, np.floating):
            raise ValueError('edit only real, nonempty 2D neural tensors')
        if edit['op'] == 'sparse_add':
            entries = edit.get('entries')
            if not isinstance(entries, list) or not 1 <= len(entries) <= 131072:
                raise ValueError('invalid sparse entries')
            coords = set()
            for item in entries:
                if not isinstance(item, dict) or not isinstance(item.get('index'), list) or len(item['index']) != 2:
                    raise ValueError('sparse entry needs 2D index')
                row, col = item['index']
                _indices([row], tensor.shape[0], 'row')
                _indices([col], tensor.shape[1], 'col')
                if (row, col) in coords:
                    raise ValueError('duplicate coordinate in sparse edit')
                coords.add((row, col))
                _finite(item.get('delta'), 'delta')
        else:
            rows = _indices(edit.get('rows'), tensor.shape[0], 'rows')
            cols = _indices(edit.get('cols'), tensor.shape[1], 'cols')
            left, right = edit.get('left'), edit.get('right')
            if not isinstance(left, list) or len(left) != len(rows) or not isinstance(right, list) or len(right) != len(cols):
                raise ValueError('rank-one factors must match index sizes')
            if len(rows) * len(cols) > 131072:
                raise ValueError('rank-one block too large')
            [_finite(x, 'left factor') for x in left]
            [_finite(x, 'right factor') for x in right]
            _finite(edit.get('scale'), 'scale')

def apply_numpy_edit(tensors: Mapping[str, np.ndarray], patch: dict):
    """Copy-on-write, gradient-free parameter editing with an exact diff audit."""
    validate_edit(tensors, patch)
    revised = dict(tensors)
    touched = set()
    for edit in patch['edits']:
        key = edit['tensor']
        if revised[key] is tensors[key]:
            revised[key] = tensors[key].copy()
        target = revised[key]
        if edit['op'] == 'sparse_add':
            for item in edit['entries']:
                i, j = item['index']
                target[i,j] = float(target[i,j]) + float(item['delta'])
                touched.add((key,i,j))
        else:
            rows,cols=edit['rows'],edit['cols']
            values=np.outer(np.array(edit['left'],dtype=np.float64),np.array(edit['right'],dtype=np.float64))*edit['scale']
            for i,row in enumerate(rows):
                for j,col in enumerate(cols):
                    target[row,col] = float(target[row,col]) + float(values[i,j])
                    touched.add((key,row,col))
    changed = sum(int(np.count_nonzero(revised[k] != v)) for k,v in tensors.items())
    total = sum(v.size for v in tensors.values())
    fraction = changed / total if total else 0.
    if fraction > patch.get('max_changed_fraction',.001)+1e-12:
        raise ValueError(f'weight edit rejected: {fraction:.7%} parameters changed')
    for k in tensors:
        if not np.all(np.isfinite(revised[k].astype(np.float64))):
            raise ValueError(f'nonfinite tensor after edit: {k}')
    differences = {k: revised[k].astype(np.float64)-v.astype(np.float64) for k,v in tensors.items() if revised[k] is not v}
    max_delta = max((float(np.abs(x).max(initial=0)) for x in differences.values()), default=0.)
    report = {'edit_method':'direct_weight_space','optimizer_used':False,'gradient_used':False,
              'changed_parameters':changed,'changed_fraction':fraction,'touched_positions':len(touched),
              'max_abs_delta':max_delta,'tensors_modified':list(differences),'total_parameters':total}
    return revised, report

def apply_torch_edit(model, patch: dict):
    """Apply the same explicit edits to a loaded open-weight LLM, in memory.

    This is an OFFLINE research action. Requires torch and model-supplied tensors,
    and never modifies the source checkpoint in place.
    """
    import torch
    parameters = dict(model.named_parameters())
    edited = set()
    # Validate all requested edits, all limits before any model modification.
    for item in patch.get('edits', []):
        key = item.get('tensor')
        if key not in parameters:
            raise ValueError(f'missing LLM tensor: {key}')
        p=parameters[key]
        # Validate shapes without copying multi-gigabyte tensors to NumPy.
        if p.ndim != 2 or not p.is_floating_point():
            raise ValueError('target must be a floating 2D tensor')
    class ShapeProxy:
        def __init__(self, p): self.shape=tuple(p.shape); self.ndim=2; self.size=p.numel(); self.dtype=np.float32
    proxy={k:ShapeProxy(v) for k,v in parameters.items() if v.ndim==2 and v.is_floating_point()}
    validate_edit(proxy,patch)
    total = sum(x.numel() for x in parameters.values())
    requested=set()
    for edit in patch['edits']:
        key=edit['tensor']
        if edit['op']=='sparse_add':
            requested.update((key,*entry['index']) for entry in edit['entries'])
        else:
            requested.update((key,i,j) for i in edit['rows'] for j in edit['cols'])
    if len(requested) / max(1,total) > patch.get('max_changed_fraction',.001):
        raise ValueError('patch exceeds model-wide change limit')
    with torch.no_grad():
        for edit in patch['edits']:
            p=parameters[edit['tensor']]
            if edit['op']=='sparse_add':
                for entry in edit['entries']:
                    i,j=entry['index'];p[i,j].add_(entry['delta'])
            else:
                rs,cs=edit['rows'],edit['cols']
                vals=torch.outer(torch.tensor(edit['left'],device=p.device,dtype=torch.float32),torch.tensor(edit['right'],device=p.device,dtype=torch.float32))*edit['scale']
                for row_id,row in enumerate(rs):
                    p[row,cs]+=vals[row_id,:].to(p.dtype)
            edited.add(edit['tensor'])
    return {'edit_method':'direct_weight_space','optimizer_used':False,'gradient_used':False,
            'requested_positions':len(requested),'changed_fraction_upper_bound':len(requested)/total,
            'tensors_modified':sorted(edited),'total_parameters':total}

def main():
    parser=argparse.ArgumentParser(description='No-grad explicit neural weight edits, with audit')
    parser.add_argument('--weights',required=True,help='Input numpy .npz with numeric weight tensors')
    parser.add_argument('--patch',required=True,help='Direct weight edit in JSON')
    parser.add_argument('--output',required=True,help='Output edited .npz (never overwrite source)')
    args=parser.parse_args()
    source=Path(args.weights).resolve(); dest=Path(args.output).resolve()
    if source==dest:parser.error('refusing to overwrite model input')
    with np.load(source,allow_pickle=False) as values:
        tensors={k:values[k] for k in values.files}
    patch=json.loads(Path(args.patch).read_text('utf-8'))
    new, report=apply_numpy_edit(tensors,patch)
    dest.parent.mkdir(parents=True,exist_ok=True)
    np.savez_compressed(dest,**new)
    report['input_sha256']=hashlib.sha256(source.read_bytes()).hexdigest()
    report['output_sha256']=hashlib.sha256(dest.read_bytes()).hexdigest()
    dest.with_suffix(dest.suffix+'.audit.json').write_text(json.dumps(report,indent=2)+'\n','utf-8')
    print(json.dumps(report,indent=2))
if __name__=='__main__': main()
