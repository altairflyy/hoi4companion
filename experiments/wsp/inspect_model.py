"""Read actual pretrained neural tensors and selected activations for WSP design.

No modification, optimization, gradients, datasets uploaded or remote requests.
Inspect only local open weights. Use evidence to propose an explicit edit JSON.
"""
from __future__ import annotations
import argparse,json,re
from pathlib import Path
import numpy as np

def summarize_matrix(matrix):
    import torch
    if matrix.ndim != 2 or not matrix.is_floating_point():
        raise ValueError('Expected a floating 2D weight matrix')
    with torch.no_grad():
        a=matrix.detach().float()
        row_norm=torch.linalg.vector_norm(a,dim=1)
        col_norm=torch.linalg.vector_norm(a,dim=0)
        return {'shape':list(matrix.shape),'dtype':str(matrix.dtype),'mean':float(a.mean()),'std':float(a.std()),
                'largest_row_norms':[[int(x),float(row_norm[x])] for x in row_norm.topk(min(12,row_norm.numel())).indices.tolist()],
                'largest_column_norms':[[int(x),float(col_norm[x])] for x in col_norm.topk(min(12,col_norm.numel())).indices.tolist()]}

def main():
    p=argparse.ArgumentParser(description='Inspect neural weights for reasoning-driven direct edits')
    p.add_argument('--model',required=True,help='Already downloaded local Qwen3.5 text model')
    p.add_argument('--match',default=r'\.mlp\.down_proj\.weight$',help='Regex selecting weight tensors')
    p.add_argument('--max-tensors',type=int,default=8)
    p.add_argument('--probe',action='append',default=[],help='One source sentence for activation probing')
    p.add_argument('--out',required=True)
    args=p.parse_args()
    root=Path(args.model).resolve()
    if not (root/'config.json').exists():p.error('local checkpoint is required')
    import torch
    from transformers import AutoProcessor,Qwen3_5ForConditionalGeneration
    processor=AutoProcessor.from_pretrained(str(root),local_files_only=True,trust_remote_code=False)
    model=Qwen3_5ForConditionalGeneration.from_pretrained(str(root),device_map='auto',dtype='auto',local_files_only=True,trust_remote_code=False)
    model.eval()
    rx=re.compile(args.match)
    selected=[(name,p) for name,p in model.named_parameters() if rx.search(name)][:args.max_tensors]
    result={'model_path':str(root),'summary':{},'probes':{}}
    for name,weights in selected:
        if weights.ndim==2:result['summary'][name]=summarize_matrix(weights)
    if args.probe:
        modules=dict(model.named_modules())
        for name,weights in selected:
            module=modules.get(name.rsplit('.weight',1)[0]);outputs=[]
            if module is None:continue
            def hook(_module,inputs):
                x=inputs[0]
                if x.ndim==3:outputs.append(x[0,-1,:].detach().float().cpu())
            handle=module.register_forward_pre_hook(hook)
            try:
                for sentence in args.probe[:16]:
                    tokens=processor.tokenizer(sentence,return_tensors='pt').to(next(model.parameters()).device)
                    with torch.inference_mode():model(**tokens,use_cache=False)
                observations=[]
                for sentence,x in zip(args.probe[:16],outputs):
                    order=torch.argsort(x.abs(),descending=True)[:16]
                    observations.append({'input':sentence,'dominant_input_dimensions':[[int(i),float(x[i])] for i in order.tolist()]})
                result['probes'][name]=observations
            finally:handle.remove()
    Path(args.out).write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n','utf8')
    print(json.dumps({'tensors':list(result['summary']),'probes':sum(len(v) for v in result['probes'].values())},indent=2))
if __name__=='__main__':main()
