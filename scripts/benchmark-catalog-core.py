"""Matched static catalog parity and timing; never launches HOI4 or imports guides."""
import argparse
import gc
import hashlib
import json
import os
from pathlib import Path
import statistics
import time

from hoi4companion.resolution import CatalogIndex,ResolutionContext
from hoi4companion._resolution_reference import CatalogIndex as Reference
from hoi4companion.research_upgrades import normal_name


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('catalog',type=Path)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    if os.environ.get('HOI4_CATALOG_CORE_VERIFY')=='1':parser.error('Disable shadow verification when measuring')
    raw=args.catalog.read_bytes();rows=json.loads(raw)
    if isinstance(rows,dict):rows=rows['entities']
    timings={};indexes={}
    for key,cls in [('python',Reference),('rust',CatalogIndex)]:
        began=time.perf_counter();indexes[key]=cls(rows);timings[key]={'build_ms':(time.perf_counter()-began)*1000}
    queries=[]
    for i in range(0,len(rows),max(1,len(rows)//500)):
        row=rows[i];context=ResolutionContext(country=next(iter(row.get('countries',[])),None),scope=row.get('scope'))
        queries.extend([(row['id'],(row['kind'],),context,True),(row['name'],(row['kind'],),context,False)])
        for alias in row.get('aliases',[])[:1]:queries.append((alias,(row['kind'],),context,False))
    for query,kinds,context,explicit in queries:
        a=indexes['python'].matches(query,kinds,context,explicit=explicit,typos=True)
        b=indexes['rust'].matches(query,kinds,context,explicit=explicit,typos=True)
        if a!=b:raise ValueError('Catalog parity failure: '+query)
    # All static labels, IDs and aliases use the identical Unicode contract.
    labels=list(dict.fromkeys(label for row in rows for label in [row['id'],row['name'],row.get('localized_name',''),*row.get('aliases',[])] if label))
    for start in range(0,len(labels),500):
        batch=labels[start:start+500]
        if indexes['rust']._native.query('normalize',texts=batch)!=[normal_name(label) for label in batch]:raise ValueError('Static catalog normalization differs')
    samples={key:[] for key in indexes}
    for batch in range(7):
        for key in (['python','rust'] if batch%2==0 else ['rust','python']):
            began=time.perf_counter()
            for query,kinds,context,explicit in queries:indexes[key].matches(query,kinds,context,explicit=explicit,typos=True)
            samples[key].append((time.perf_counter()-began)*1000/len(queries))
    for key in timings:timings[key].update(query_median_ms=statistics.median(samples[key]),query_batch_mean_ms=samples[key])
    result={'schema_version':1,'basis':'offline_cached_catalog_no_gameplay_or_hud_proof','catalog_sha256':hashlib.sha256(raw).hexdigest(),
        'rows':len(rows),'query_parity_cases':len(queries),'normalization_labels':len(labels),'outputs_equal':True,'timing':timings}
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(result,indent=2)+'\n',encoding='utf8')
    print(json.dumps({key:value for key,value in result.items() if key!='timing'}));print(json.dumps(timings))
    indexes.clear();gc.collect()


if __name__=='__main__':main()
