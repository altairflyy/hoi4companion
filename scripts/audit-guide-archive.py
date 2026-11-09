"""Audit Word collection imports against the locally installed game catalog."""
import argparse
import json
from pathlib import Path
from collections import Counter

from hoi4companion.api import Store
from hoi4companion.word_guides import list_documents, prepare_document
from hoi4companion.guide_authoring import compile_document
from hoi4companion.association_review import reference_catalog,review_associations,review_markdown
from hoi4companion.guide_compatibility import import_availability


def runtime_parity(guide):
    """Synthetic evidence only, never a native observation or persisted run."""
    from hoi4companion.guide_core import evaluate
    from hoi4companion.guide_runtime import evaluate_python
    scope=guide.get('applies_to',{})
    country=scope.get('country') or 'TST'
    tree=scope.get('tree_id')
    rows=[row for lane in guide['tracks'].values() for row in lane]
    focus=[r['action'].get('entity',{}).get('id') for r in rows if r['action']['kind']=='select_focus']
    research=[r['action'].get('entity',{}).get('id') for r in rows if r['action']['kind']=='research']
    def state(*,completed=False,wrong=False):
        values={'country':'ZZZ' if wrong and country!='ZZZ' else 'TST' if wrong else country,
            'focus.tree':tree,'focus.active':[],'focus.completed':focus if completed else [],
            'research.active':[],'research.completed':research if completed else [],
            'research.slots':[{'slot':n,'technology_id':None} for n in range(3)],
            'politics.power':100,'economy.army_experience':0}
        return {'schema_version':1,'usable':True,'session_id':'offline-audit',
                'facts':{key:{'status':'observed','source':'synthetic_fixture','value':value}
                         for key,value in values.items() if value is not None}}
    cases={'preview':{'schema_version':1,'usable':False,'facts':{}},
        'unknown':{'schema_version':1,'usable':True,'facts':{}},
        'initial':state(),'completed_membership':state(completed=True),'wrong_country':state(wrong=True)}
    for name,value in cases.items():
        if evaluate(guide,value)!=evaluate_python(guide,value):
            raise ValueError('Runtime comparison differs: '+name)
    return {'cases':len(cases),'equal':True,'basis':'synthetic_not_live'}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive',type=Path)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--review-output',type=Path,help='Write unresolved identities and proposals as Markdown and JSON; never installs them.')
    parser.add_argument('--runtime-parity',action='store_true',help='Compare native/reference decisions with offline synthetic evidence; never installs guides.')
    args=parser.parse_args()
    store=Store(); importer=store.importer(); results=[]
    compatibility=store.guide_compatibility()
    with args.archive.open('rb') as stream:
        inventory=list_documents(stream)
        for entry in inventory['documents']:
            parts=Path(entry).parts
            if not any(p.casefold()=='country guides' for p in parts) or any(p.casefold()=='tools' for p in parts):continue
            try:
                stream.seek(0); packet=prepare_document(stream,entry)
                document=compile_document(packet['document'])
                result=importer.preview(document['guide'],document['sources'])
                source_rows={r['key']:r for lane in document['guide']['tracks'].values() for r in lane}
                unresolved=[]
                for resolution in result['resolutions']:
                    if resolution['status']=='resolved':continue
                    resolution['evidence']=source_rows.get(resolution['key'],{}).get('evidence',[])
                    unresolved.append(resolution)
                content=compatibility.assess(document['guide']) if compatibility else {'blocked':False,'status':'unknown'}
                availability=import_availability(result,content)
                parity=runtime_parity(store._runtime_definition(result['guide'])) if args.runtime_parity and result.get('guide') else None
                results.append({'title':Path(entry).name,'valid':result['valid'],
                    'compatibility':content,'availability':availability,
                    'errors':[] if content['blocked'] else result['errors'],'warnings':result['warnings'],
                    **({'unavailable_content_diagnostics':result['errors']} if content['blocked'] else {}),
                    'applies_to':result['guide']['applies_to'] if result.get('guide') else {},
                    'resolved_count':sum(r['status']=='resolved' for r in result['resolutions']),
                    'route_recoveries':[{**r,'source_query':source_rows[r['key']]['action']['entity']['id']}
                        for r in result['resolutions'] if r.get('matching_basis')=='guide_required_ancestor'],
                    'word_recoveries':[{**r,'source_query':source_rows[r['key']]['action']['entity']['id']}
                        for r in result['resolutions'] if r.get('matching_basis')=='focus_one_word_variant'],
                    'unresolved':[] if content['blocked'] else unresolved,
                    'operations':dict(Counter(r['monitoring'] for r in result.get('operations',[]))),
                    **({'runtime_parity':parity} if parity is not None else {})})
            except (ValueError,OSError) as exc:
                results.append({'title':Path(entry).name,'valid':False,'errors':[str(exc)]})
            status=results[-1].get('availability',{}).get('status','needs_revision')
            print(f"{len(results)}: {status.upper()} {Path(entry).name}",flush=True)
    report={'schema_version':1,'basis':'installed_catalog_import_validation_not_live_verification',
            'guides':len(results),'valid':sum(r['valid'] for r in results),
            'ready':sum(r.get('availability',{}).get('status')=='ready' for r in results),
            'dlc_unavailable':sum(r.get('availability',{}).get('status')=='dlc_unavailable' for r in results),
            'needs_revision':sum(r.get('availability',{}).get('status','needs_revision')=='needs_revision' for r in results),
            'results':results}
    if args.runtime_parity:
        report['runtime_parity']={'guides':sum('runtime_parity' in r for r in results),
            'cases':sum(r.get('runtime_parity',{}).get('cases',0) for r in results),
            'basis':'synthetic_not_live'}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    print(f"Ready: {report['ready']}; DLC unavailable: {report['dlc_unavailable']}; review: {report['needs_revision']}; report: {args.output.resolve()}")
    if args.review_output:
        reference,warnings=reference_catalog(store)
        review=review_associations(report,reference);review['warnings']=warnings
        args.review_output.parent.mkdir(parents=True,exist_ok=True)
        args.review_output.write_text(review_markdown(review),encoding='utf8')
        args.review_output.with_suffix('.json').write_text(json.dumps(review,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
        print(f'Review: {args.review_output.resolve()} — {review["counts"]}')


if __name__=='__main__':main()
