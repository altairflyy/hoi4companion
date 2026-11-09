"""Matched, offline guide evaluations. Does not read HOI4 or campaign storage."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import sys
import time

from hoi4companion.guide_core import clear_compiled, engine_info, evaluate
from hoi4companion.guide_runtime import evaluate_python


def measure(guide, state, batches, iterations):
    # Warm-up is excluded; retain the same definition and state for both engines.
    engines={'python':evaluate_python,'native':evaluate}
    for fn in engines.values():
        for _ in range(3):fn(guide,state)
    samples={key:[] for key in engines}
    for batch in range(batches):
        order=list(engines) if batch%2==0 else list(reversed(engines))
        for key in order:
            began=time.perf_counter_ns()
            for _ in range(iterations):engines[key](guide,state)
            samples[key].append((time.perf_counter_ns()-began)/iterations/1_000_000)
    return {key:{'median_ms':statistics.median(values),'batch_mean_ms':values,
            'batches':batches,'iterations_per_batch':iterations} for key,values in samples.items()}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('guide',type=Path)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--batches',type=int,default=11)
    parser.add_argument('--iterations',type=int,default=10)
    args=parser.parse_args()
    if not 3<=args.batches<=100 or not 1<=args.iterations<=1000:parser.error('Invalid measurement bounds')
    if os.environ.get('HOI4_GUIDE_CORE_VERIFY')=='1':parser.error('Disable shadow comparison for timing')
    source=args.guide.read_bytes();guide=json.loads(source)
    state={'schema_version':1,'usable':False,'facts':{}}
    expected=evaluate_python(guide,state);clear_compiled()
    began=time.perf_counter_ns();actual=evaluate(guide,state)
    cold_ms=(time.perf_counter_ns()-began)/1_000_000
    if actual!=expected:raise ValueError('Native/reference outputs differ; timings not admitted')
    report={'schema_version':1,'scope':'offline_preview_no_hud_or_live_timing',
        'measurement_order':'alternating_paired_batches','python_version':sys.version.split()[0],
        'engine':engine_info(),'guide_sha256':hashlib.sha256(source).hexdigest(),
        'steps':sum(map(len,guide['tracks'].values())),'outputs_equal':True,
        'native_first_evaluation_ms':cold_ms,**measure(guide,state,args.batches,args.iterations)}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({key:value for key,value in report.items() if key not in ('python','native')}))
    print('Python median: %.3f ms; native median: %.3f ms' % (report['python']['median_ms'],report['native']['median_ms']))


if __name__=='__main__':main()
