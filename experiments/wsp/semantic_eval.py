"""Strict semantic-IR grader. Rejects invented game IDs and excess fields."""
from __future__ import annotations
import argparse,json
from pathlib import Path

REQUIRED_KEYS={'operations','unresolved'}
OP_KEYS={'domain','operation','target','quantity','when','until','depends_on','targets','location','resource_cost','priority','details'}


def canonical(v):
    if isinstance(v,dict): return {k:canonical(val) for k,val in sorted(v.items())}
    if isinstance(v,list): return [canonical(x) for x in v]
    return v

def score(expected:dict, actual:dict):
    """Exact match is intentionally conservative; neither a partial nor an unsafe parse passes."""
    if not isinstance(actual,dict) or not isinstance(actual.get('operations'),list) or not isinstance(actual.get('unresolved'),list):
        return {'exact':False,'valid':False,'has_extra_fields':True,'abstained':False,'operation_count':0}
    extras = set(actual)-REQUIRED_KEYS
    for op in actual['operations']:
        if not isinstance(op,dict) or not isinstance(op.get('domain'),str) or not isinstance(op.get('operation'),str):
            return {'exact':False,'valid':False,'has_extra_fields':True,'abstained':False,'operation_count':0}
        extras |= set(op)-OP_KEYS
    valid = not extras and all(isinstance(x,str) for x in actual['unresolved'])
    return {'exact':valid and canonical(actual)==canonical(expected), 'valid':valid,
            'has_extra_fields':bool(extras), 'abstained':bool(actual['unresolved']) or not bool(actual['operations']),
            'operation_count':len(actual['operations'])}

def evaluate(reference, predictions, split=None):
    cases={v['id']:v for v in reference if split is None or v['split']==split}
    pred={v['id']:v['result'] for v in predictions}
    details={key:score(value['expected'],pred.get(key)) for key,value in cases.items()}
    count=len(details)
    return {'n':count,'exact':sum(x['exact'] for x in details.values()),
            'exact_rate':sum(x['exact'] for x in details.values())/count if count else None,
            'valid_rate':sum(x['valid'] for x in details.values())/count if count else None,
            'abstentions':sum(x['abstained'] for x in details.values()),
            'extra_field_failures':sum(x['has_extra_fields'] for x in details.values()),
            'cases':details}

def read_jsonl(path):
    return [json.loads(x) for x in Path(path).read_text('utf-8').splitlines() if x.strip()]

def main():
    p=argparse.ArgumentParser();p.add_argument('--reference',required=True);p.add_argument('--predictions',required=True);p.add_argument('--split',default='heldout',choices=['development','heldout','all']);p.add_argument('--output');args=p.parse_args()
    report=evaluate(read_jsonl(args.reference),read_jsonl(args.predictions),None if args.split=='all' else args.split)
    text=json.dumps(report,indent=2,ensure_ascii=False)
    if args.output:Path(args.output).write_text(text+'\n','utf-8')
    print(text)
if __name__=='__main__':main()
