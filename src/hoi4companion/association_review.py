"""Read-only association review; proposals never enter the runtime catalog."""
from collections import Counter
from copy import deepcopy

from .resolution import CatalogIndex, ResolutionContext
from .guide_importer import catalog_entities
from .focus import load_trees, load_continuous_focuses
from .installation import collect_files
from .eligibility import selector_tags
from .jomini_parser import parse_file
from .clausewitz import Node
from .scripted_names import NameExpander


def reference_catalog(store):
    """Include unavailable definitions as evidence, without enabling them."""
    active=store.import_catalog()
    active_ids={(e['kind'],e['id'],e.get('scope')) for e in active}
    files,warnings=collect_files(store.game_root,store.mod_roots,'common/national_focus','.txt')
    trees,issues=load_trees(files,store.labels);warnings+=issues
    for tree in trees.values():
        if isinstance(tree.country_selector,Node):
            tree.eligible_countries=sorted(selector_tags(tree.country_selector,store.scripts))
    files,issues=collect_files(store.game_root,store.mod_roots,'common/continuous_focus','.txt');warnings+=issues
    continuous,issues=load_continuous_focuses(files,store.labels);warnings+=issues
    techs={}
    files,issues=collect_files(store.game_root,store.mod_roots,'common/technologies','.txt');warnings+=issues
    for path in files:
        try:
            for group in parse_file(path).blocks('technologies'):
                for key,node in group.entries:
                    if not isinstance(node,Node) or key.startswith('@'):continue
                    techs[key]={'name':store.labels.get(key,key),'source':str(path),
                        'paths':[{'target':p.scalar('leads_to_tech')} for p in node.blocks('path') if p.scalar('leads_to_tech')],
                        'unlocks':[{'kind':'Equipaggiamento','id':value} for block in node.blocks('enable_equipments')
                                   for _,value in block.entries if isinstance(value,str)]}
        except (ValueError,OSError,RecursionError) as exc:warnings.append(f'{path.name}: {exc}')
    rows=catalog_entities(trees,techs,store.labels,store.reference_labels,continuous_focuses=continuous)
    excluded={e['id']:e['reason'] for e in store.excluded_trees}
    excluded_branches={(e['scope'],e['id']):e['reason'] for e in getattr(store,'excluded_focuses',[])}
    names=NameExpander(store.scripted_names,store.reference_labels)
    for row in rows:
        row['availability']='available' if (row['kind'],row['id'],row.get('scope')) in active_ids else 'outside_active_catalog'
        row['source']=(continuous[row['id']].source if row.get('continuous') else
                       trees[row['scope']].source if row['kind']=='focus' else techs[row['id']]['source'])
        if (row.get('scope'),row['id']) in excluded_branches:row['availability_reason']=excluded_branches[row.get('scope'),row['id']]
        elif row.get('scope') in excluded:row['availability_reason']=excluded[row['scope']]
        elif row['availability']!='available':row['availability_reason']='Definition exists, but is excluded by the current technology/DLC catalog.'
        aliases=names.expand(row['name'],countries=row.get('countries',()))
        row['aliases']=list(dict.fromkeys([*row.get('aliases',[]),*aliases]))
    existing={(r['kind'],r['id'],r.get('scope')) for r in rows}
    rows.extend({**e,'availability':'available'} for e in active
                if (e['kind'],e['id'],e.get('scope')) not in existing)
    return store.resolution_index().with_entries(rows),warnings


def review_associations(report,reference):
    """Group every unresolved occurrence, retaining conflicts and evidence."""
    grouped={}
    for guide in report['results']:
        if guide.get('compatibility',{}).get('blocked') is True:
            continue
        queries={'tracks':{'focus':[{'action':{'kind':'select_focus','entity':{'kind':'focus','id':item['query']}}}
                 for item in guide.get('unresolved',[]) if item.get('kind')=='focus']}}
        source_country=reference.country_hint(guide['title'],queries)
        for issue in guide.get('unresolved',[]):
            kind=issue.get('kind') or next((r['kind'] for r in issue.get('candidates',[])),None)
            context=deepcopy(issue.get('context',{}))
            if not context.get('country') and source_country:context['country']=source_country
            key=(kind,issue['query'],context.get('country'),context.get('scope'))
            if key not in grouped:
                # Do not constrain to an active tree: the source may require an
                # unavailable DLC tree. Keep that difference visible for review.
                country=ResolutionContext(country=context.get('country'))
                exact=reference.matches(issue['query'],(kind,),country) if kind else []
                proposals=deepcopy(exact) if exact else reference.suggest(issue['query'],(kind,),country,limit=4) if kind else []
                if not proposals:proposals=deepcopy(issue.get('candidates',[]))
                status=('documented_identity_outside_active_catalog' if len(exact)==1 and exact[0].get('availability')!='available' else
                        'documented_identity_needs_context' if exact else 'proposals_require_confirmation' if proposals else 'no_supported_candidate')
                grouped[key]={'query':issue['query'],'kind':kind,'context':context,'status':status,
                              'candidates':proposals,'occurrences':[]}
            grouped[key]['occurrences'].append({'guide':guide['title'],'step':issue['key'],
                'reason':issue.get('reason',issue['status']),'evidence':issue.get('evidence',[])})
    compatibility_rows=[row for row in grouped.values() if row['status']=='documented_identity_outside_active_catalog']
    results=[row for row in grouped.values() if row['status']!='documented_identity_outside_active_catalog']
    return {'schema_version':1,'basis':'local_script_identity_and_review_proposals_not_live_verification',
            'counts':dict(Counter(r['status'] for r in results)),'results':results,
            'catalog_compatibility':compatibility_rows,
            'other_import_errors':[{'guide':g['title'],'errors':[e for e in g.get('errors',[])
                if not any(e.startswith(item['key']+':') for item in g.get('unresolved',[]))]}
                for g in report['results'] if g.get('compatibility',{}).get('blocked') is not True and any(not any(e.startswith(item['key']+':')
                    for item in g.get('unresolved',[])) for e in g.get('errors',[]))]}


def review_markdown(review):
    labels={'documented_identity_outside_active_catalog':'ID documentato, fuori dal catalogo attivo',
            'documented_identity_needs_context':'ID documentati: verificare percorso o ambito',
            'proposals_require_confirmation':'Proposte da confermare','no_supported_candidate':'Nessuna corrispondenza nominale attendibile'}
    def text(value):return str(value).replace('`','\\`').replace('\n',' ')
    lines=['# Associazioni da revisionare','',
        '## Conteggi','',
        *[f'- {label}: **{review["counts"].get(status,0)}** diciture/ambiti distinti.' for status,label in labels.items() if status!='documented_identity_outside_active_catalog' and review['counts'].get(status,0)],
        f'- Altre identità fuori dal catalogo attivo (compatibilità, non associazioni da confermare): **{len(review.get("catalog_compatibility",[]))}**.',
        '',
        'La lista contiene solo le associazioni da revisionare. Le proposte per somiglianza non vengono applicate automaticamente.',
        '', 'Nessuna guida è stata modificata. Nessuna verifica live è inclusa in questo rapporto.','']
    if not review['results']:
        lines.extend(['**Nessuna associazione da confermare.**',''])
    for status,label in labels.items():
        if status=='documented_identity_outside_active_catalog':continue
        rows=[r for r in review['results'] if r['status']==status]
        if not rows:continue
        lines.extend([f'## {label} ({len(rows)})',''])
        for row in rows:
            lines.extend([f'### {text(row["query"])} · {text(row["kind"])}',''])
            reason=('La guida indica alternative.' if '/' in row['query'] else
                    'Nessun ID trovato.' if not row['candidates'] else
                    'Più ID possibili: serve scegliere il ramo o il livello.' if status=='documented_identity_needs_context' else
                    'Probabile refuso o nome abbreviato: confronta la proposta con il testo originale.')
            lines.extend(['**Motivo:** '+reason,''])
            for candidate in row['candidates']:
                label=candidate.get('suggestion',{}).get('matched_label',candidate['name'])
                lines.append(f'- **{text(label)}** → `{text(candidate["id"])}`')
            lines.extend(['','Presente in:',''])
            for occurrence in row['occurrences']:
                lines.append(f'- {text(occurrence["guide"])} · `{text(occurrence["step"])}`')
                context=occurrence.get('source_context',{})
                if context:
                    lines.extend(['',f'  Colonna **{text(context.get("section",""))}**, voce {context.get("position_in_section","?")}.',
                        '',f'  Testo originale: **{text(context.get("original_line",row["query"]))}**.',
                        '', '  Prima: '+(' → '.join(text(v) for v in context.get('previous',[])) or 'inizio della colonna')+'.',
                        '', '  Dopo: '+(' → '.join(text(v) for v in context.get('next',[])) or 'fine della colonna')+'.',''])
            lines.append('')
    lines.extend(['## Altre incompatibilità di catalogo — non richiedono conferma dell’associazione',''])
    for item in review.get('catalog_compatibility',[]):
        lines.append(f'- {text(item["query"])}: '+', '.join('`'+text(c['id'])+'`' for c in item['candidates']))
    lines.extend(['','## Altri blocchi di importazione',''])
    for item in review.get('other_import_errors',[]):
        lines.extend([f'### {text(item["guide"])}','',*[f'- {text(e)}' for e in item['errors']],''])
    return '\n'.join(lines)+'\n'
