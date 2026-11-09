"""Guide Importer v1: deterministic resolution/validation around untrusted AI output."""
from __future__ import annotations
from .source_coverage import coverage_report, inventory_items
from .guide_sequence import sequence_report

from copy import deepcopy
from contextlib import nullcontext
import hashlib
import json
import math
import os
from pathlib import Path
import re
import tempfile
from urllib.error import HTTPError, URLError
from urllib.request import Request, build_opener, HTTPRedirectHandler

from .guide import normalise_guide, GUIDES, LOCAL_GUIDES
from .import_sources import MAX_TEXT, MAX_BYTES
from .research_upgrades import normal_name, upgrade_successors, resolve_research_upgrades
from .tooltips import localise
from .focus_resolution import resolve_focus_routes, infer_focus_scope
from ._operation_import_reference import bind_operations
from .resolution import catalog_index, ResolutionContext


from .guide_importer import validate_sources

def preview(guide, sources, entities, research_context=None):
    report = {'import_schema_version': 1, 'valid': False, 'guide': None,
              'draft': deepcopy(guide) if isinstance(guide, dict) else None,
              'errors': [], 'warnings': [], 'resolutions': [],
              'visual_review': {'count': 0, 'step_keys': []}}
    try:
        index=catalog_index(entities)
        entities=index
        acquired = validate_sources(sources)
        if not isinstance(guide, dict) or not isinstance(guide.get('id'), str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,160}', guide['id']):
            raise ValueError('ID guida necessario: usa lettere, numeri, trattino o underscore')
        json.dumps(guide, allow_nan=False)
        if not isinstance(guide.get('title'), str) or not guide['title'].strip():
            raise ValueError('Titolo guida necessario')
        value = normalise_guide(guide)
        if not value['applies_to'].get('country'):
            country=index.country_hint(value['title'],value)
            if country:
                value['applies_to']['country']=country
                report['warnings'].append(f'Nazione {country} riscontrata nel titolo e nei focus della guida; non dedotta dalla partita corrente.')
        inferred_scope = infer_focus_scope(value, index)
        if inferred_scope:
            value['applies_to']['tree_id'] = inferred_scope
            report['warnings'].append(f'Albero focus dedotto dai nomi presenti nella guida: {inferred_scope}. Verifica che corrisponda alla nazione e alla versione della guida.')
        if not isinstance(value.get('subtitle'), str) or not isinstance(value.get('assumptions'), list) or any(
                not isinstance(item, str) for item in value['assumptions']):
            raise ValueError('Sottotitolo o elenco dei limiti non valido')
        if value.get('milestones') != []:
            raise ValueError('Importa i checkpoint come passaggi operativi con evidenza, non come milestones legacy')
        if 'source' in value and (not isinstance(value['source'], dict) or
                not isinstance(value['source'].get('chapters', []), list) or
                any(not isinstance(item, dict) or not isinstance(item.get('title'), str) or
                    type(item.get('start_time')) not in (int, float) for item in value['source'].get('chapters', []))):
            raise ValueError('Metadati fonte legacy non validi')
        rows = [row for lane in value['tracks'].values() for row in lane]
        if not rows:
            raise ValueError('La guida non contiene istruzioni')
        # Retain local document text so material not converted into steps remains
        # available after installation. Web/video transcripts stay in the draft.
        declared = {item['id'] for item in value.get('sources', [])}
        if not declared or not declared <= acquired.keys():
            raise ValueError('La guida cita fonti non acquisite')
        value['sources'] = [{k: src[k] for k in
                             (('id', 'type', 'title', 'url', 'content_sha256', 'text')
                              if src.get('type') == 'file' else ('id', 'type', 'title', 'url', 'content_sha256')) if k in src}
                            for key, src in acquired.items() if key in declared]
        value.pop('default', None)
        visual_steps=set()
        def supported_visual(item):
            src=acquired.get(item.get('source_id'))
            quote=item.get('quote')
            if (item.get('basis')!='visual' or not src or src.get('acquisition')!='external_ai'
                    or not isinstance(quote,str) or not quote.strip() or quote not in src['text']):
                return False
            if 'timestamp_seconds' not in item:
                return src['type']!='youtube'
            timestamp=item['timestamp_seconds']
            return any(isinstance(seg,dict) and type(seg.get('start')) in (int,float)
                       and abs(seg['start']-timestamp)<=1 and quote in seg.get('text','')
                       for seg in src.get('segments',[]))
        # Word research columns can also name a special project without the
        # editorial "Special Project" suffix. Only an exact, unique project
        # identity and zero technology identities permit this category repair.
        for row in rows:
            action=row['action']; ref=action.get('entity',{})
            query=ref.get('id','')
            params=action.get('params',{})
            if (action['kind']=='research' and '_' not in query and 'upgrade_from' not in params
                    and params.get('interpretation')!='next_upgrade' and not index.matches(query,('technology',))):
                projects=index.matches(query,('project',))
                if len(projects)==1:
                    action.update(kind='special_project',entity={'kind':'project','id':projects[0]['id']})
                    row.update(kind='project',target=projects[0]['id'])
                    report['warnings'].append(f'{row["key"]}: {query} è un progetto speciale nel catalogo, non una tecnologia ordinaria.')
        from .conditional_focus import compile_conditional_focuses
        conditional_focuses = compile_conditional_focuses(value,index)
        for key, item in conditional_focuses.items():
            if item['condition_basis']=='source_text_unverifiable':
                report['warnings'].append(f'{key}: Conditional alternative resolved; source condition "{item["condition_text"]}" has no measurable criterion and remains unverifiable. No automatic recommendation is made from this phrase.')
        upgrades = resolve_research_upgrades(value, entities, research_context, resolver=index)
        if research_context:
            report['research_context'] = deepcopy(research_context)
        focus_matches, report['focus_route'], route_errors = resolve_focus_routes(value,entities)
        report['focus_sequence'] = report['focus_route']['sequence']
        report['errors'].extend(route_errors)
        for row in rows:
            key = row['key']
            params=row['action'].get('params',{})
            hint=params.get('category_hint')
            if isinstance(hint,str) and hint!=params.get('category'):
                report['warnings'].append(f'{key}: category "{hint}" grouped under "{params.get("category","checkpoints")}"; the original label is retained and does not define live monitoring.')
            evidence = row.get('evidence', [])
            if not evidence:
                report['errors'].append(f'{key}: manca il riscontro nella fonte')
            for item in evidence:
                src = acquired.get(item['source_id'])
                quote = item.get('quote')
                visual = item['basis'] == 'visual' and src and src.get('acquisition') == 'external_ai'
                if (item['basis'] != 'explicit' and not visual) or not isinstance(quote, str) or not quote.strip() or not src or quote not in src['text']:
                    report['errors'].append(f'{key}: citazione non esplicita o non presente nel testo acquisito')
                if visual:
                    report['warnings'].append(f'{key}: osservazione visiva dichiarata dall’IA esterna, da confrontare con la fonte')
                    if supported_visual(item):visual_steps.add(key)
                    if src['type'] == 'youtube' and 'timestamp_seconds' not in item:
                        report['errors'].append(f'{key}: osservazione visiva del video priva di timestamp')
                if 'timestamp_seconds' in item:
                    segments = src.get('segments', []) if src else []
                    if not any(isinstance(seg, dict) and isinstance(seg.get('text'), str) and
                               type(seg.get('start')) in (int, float) and
                               abs(seg['start'] - item['timestamp_seconds']) <= 1 and
                               isinstance(quote, str) and quote in seg['text'] for seg in segments):
                        report['errors'].append(f'{key}: timestamp non riscontrato nei sottotitoli acquisiti')
            action = row['action']; ref = action.get('entity')
            if action['kind'] in ('select_focus', 'research', 'special_project'):
                scope = ref.get('scope') or (value['applies_to'].get('tree_id') if ref['kind'] == 'focus' else None)
                country = value['applies_to'].get('country') if ref['kind'] == 'focus' else None
                context=ResolutionContext(country=country,scope=scope)
                candidates=index.rows((ref['kind'],),context)
                needle = normal_name(ref['id'])
                upgrade = upgrades.get(key)
                source_binding=focus_matches.get(key)
                if source_binding and not source_binding['locked']:
                    ref['id']=source_binding['source_query']
                original_query = ref['id']
                exact = index.matches(original_query,(ref['kind'],),context)
                matches = exact or index.matches(original_query,(ref['kind'],),context,typos=True)
                basis = 'catalog_name_or_id' if exact else 'minor_typo'
                if row.get('research_choice'):basis='research_branch_choice'
                supporting = []
                route_match = focus_matches.get(key)
                if route_match:
                    matches = route_match['matches']
                    supporting = route_match['supporting_focus_ids']
                    if route_match['inferred']:
                        basis = 'guide_required_ancestor' if route_match.get('recovery') else 'guide_prerequisites'
                    if route_match.get('government_context'):basis='guide_government_branch'
                    if route_match.get('word_variant'):basis='focus_one_word_variant'
                    if route_match.get('recovery',{}).get('method')=='unique_global_sequence':basis='focus_sequence_constraints'
                if upgrade and upgrade['reason'] not in ('upgrade_resolved', 'upgrade_live'):
                    matches = []
                if len(matches) == 1:
                    match = matches[0]; ref['id'] = match['id']
                    if match.get('continuous'):
                        action.setdefault('params',{})['focus_mode']='continuous'
                        from .resolution import optional_focus_name
                        if optional_focus_name(original_query):action['params']['focus_optional']=True
                    elif action['kind']=='select_focus':
                        action.get('params',{}).pop('focus_mode',None)
                        action.get('params',{}).pop('focus_optional',None)
                    if match.get('scope'): ref['scope'] = match['scope']
                    row['target'], row['name'] = match['id'], match.get('localized_name', match['name'])
                    resolution = {'key': key, 'status': 'resolved', 'entity': deepcopy(ref), 'candidates': matches,
                                  'matching_basis':basis}
                    if key in conditional_focuses:
                        resolution.update(matching_basis='conditional_focus',association_evidence=conditional_focuses[key])
                    if basis == 'focus_sequence_constraints':
                        report['warnings'].append(f'{key}: reconstructed at source position {route_match["source_position"]}: {original_query} → {match["name"]} ({match["id"]}); identity evidence only, not live completion.')
                    elif not exact:
                        report['warnings'].append(f'{key}: refuso riconosciuto: {original_query} → {match["name"]} ({match["id"]})')
                    if supporting:
                        resolution['supporting_focus_ids'] = supporting
                    if route_match and route_match.get('recovery'):
                        resolution['association_evidence']=deepcopy(route_match['recovery'])
                    if route_match and route_match.get('government_context'):
                        resolution['association_evidence']=deepcopy(route_match['government_context'])
                    if route_match and route_match.get('word_variant'):
                        resolution['association_evidence']=deepcopy(route_match['word_variant'])
                else:
                    suggested = matches or (upgrade['candidates'] if upgrade else []) or index.suggest(original_query,(ref['kind'],),context)
                    if route_match and route_match.get('probable_candidates'):
                        suggested=route_match['probable_candidates']
                    if route_match and route_match['has_name_matches']:
                        suggested = route_match['viable_candidates']
                    resolution = {'key': key, 'status': 'ambiguous' if matches else 'unresolved', 'query': ref['id'],
                                  'kind':ref['kind'],'context':{'country':country,'scope':scope},'candidates': suggested[:12]}
                    if route_match and route_match['unresolved_route']:
                        resolution['status'] = 'unresolved_route'
                    report['errors'].append(f'{key}: ID {ref["kind"]} ambiguo o non risolto ({ref["id"]})')
                    if resolution['status']=='unresolved_route':
                        report['errors'][-1] = f'{key}: percorso focus non verificabile: prerequisiti mancanti o ciclici ({ref["id"]})'
                if route_match and route_match['excluded_candidates']:
                    resolution['excluded_candidates'] = deepcopy(route_match['excluded_candidates'])
                if route_match:
                    resolution.update(classification=route_match['classification'],
                                      source_position=route_match['source_position'],
                                      source_query=route_match['source_query'])
                    row['focus_resolution']={'schema_version':1,'position':route_match['source_position'],
                        'query':route_match['source_query'],'classification':route_match['classification'],
                        'entity':deepcopy(ref),'locked':route_match['locked']}
                excluded=index.excluded(original_query,(ref['kind'],),context)
                if excluded:resolution['context_exclusions']=excluded
                if upgrade:
                    resolution.update({field:upgrade[field] for field in ('query','interpretation','upgrade_from','reason')})
                    if upgrade['reason'] == 'upgrade_live':
                        report['warnings'].append(f'{key}: ramo UP identificato; il livello verrà associato ai dati di ricerca della partita, non allo scenario iniziale.')
                    if upgrade['reason'] not in ('upgrade_resolved', 'upgrade_live'):
                        explanations = {'upgrade_baseline_unknown':'specifica il livello di partenza o upgrade_from',
                                        'upgrade_end_of_line':'il ramo non ha un livello successivo nel catalogo',
                                        'upgrade_branch_ambiguous':'il ramo ha più successori: specifica quello previsto',
                                        'upgrade_target_missing':'il successore non è nel catalogo compatibile',
                                        'upgrade_cycle_or_repeated_target':'il successore ripete un livello precedente'}
                        report['errors'][-1] = f'{key}: UP = prossimo livello; ' + explanations[upgrade['reason']]
                report['resolutions'].append(resolution)
        # Bind live criteria only after canonical native identities have been
        # resolved (a project name is not an explicit native project ID).
        for row in rows:
            if not row['action'].get('params',{}).get('focus_optional') or row.get('optional_before'):continue
            hint=re.search(r'\buntil\s+(.+?)\s+(?:is\s+)?available\b',str(row.get('note',''))+' '+str(row.get('title','')),re.I)
            if not hint:continue
            name=hint[1].strip(' \"“”\'')
            matches=index.matches(name,('focus',),ResolutionContext(country=value['applies_to'].get('country'),scope=value['applies_to'].get('tree_id')))
            if len(matches)==1 and any(s['action']['kind']=='select_focus' and s.get('target')==matches[0]['id'] for s in rows):
                row['optional_before']=matches[0]['id']
        from ._informal_operations_reference import promote_operations
        promotions=promote_operations(value,index)
        for item in promotions:
            if item['unverifiable_timing']:
                report['warnings'].append(f'{item["key"]}: Source actions resolved, but the informal timing condition remains unverifiable; no phase is inferred from missing data.')
        rows=[row for lane in value['tracks'].values() for row in lane]
        report['operations'] = bind_operations(value,entities)
        for src in sources:
            report['warnings'].extend(src.get('warnings', []))
        report['coverage'] = coverage_report(value, acquired)
        all_keys={row['key'] for row in rows}
        for item in inventory_items(value) or []:
            if not item.get('excluded_reason') and supported_visual(item):
                visual_steps.update(key for key in item.get('step_keys',[]) if key in all_keys)
        report['visual_review']={'count':len(visual_steps),'step_keys':sorted(visual_steps)}
        report['errors'].extend('Copertura: ' + item['message'] for item in report['coverage']['issues'])
        report['guide'] = normalise_guide(value)
        report['sequence_report'] = sequence_report(report['guide'])
        report['valid'] = not report['errors']
    except (ValueError, TypeError, KeyError, AttributeError, RecursionError) as exc:
        report['errors'].append(str(exc) or 'Documento non valido')
    return deepcopy(report)  # Callers cannot corrupt the reusable catalog index.

