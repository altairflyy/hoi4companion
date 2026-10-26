"""Editorial accounting, not a claim that an AI saw the entire source."""
import math


def inventory_items(guide):
    inventory = guide.get('source_inventory')
    if inventory is None:
        return None
    if (not isinstance(inventory, dict) or type(inventory.get('schema_version')) is not int
            or inventory['schema_version'] != 1 or set(inventory) != {'schema_version', 'items'}
            or not isinstance(inventory['items'], list) or not 0 < len(inventory['items']) <= 4096):
        raise ValueError('Inventario fonti v1: richiesto un elenco di 1–4096 istruzioni')
    seen = set()
    for item in inventory['items']:
        if not isinstance(item, dict) or set(item) - {'id', 'source_id', 'quote', 'timestamp_seconds', 'step_keys', 'excluded_reason'}:
            raise ValueError('Inventario fonti: campi non validi')
        for field, limit in [('id', 160), ('source_id', 160), ('quote', 4096)]:
            if not isinstance(item.get(field), str) or not item[field].strip() or len(item[field]) > limit:
                raise ValueError('Inventario fonti: campo non valido ' + field)
        if item['id'] in seen:
            raise ValueError('Inventario fonti: ID duplicato ' + item['id'])
        seen.add(item['id'])
        keys = item.get('step_keys', [])
        if (not isinstance(keys, list) or len(keys) > 4096
                or any(not isinstance(k, str) or not 0 < len(k) <= 160 for k in keys)
                or len(set(keys)) != len(keys)):
            raise ValueError('Inventario fonti: step_keys non validi')
        if 'excluded_reason' in item and (not isinstance(item['excluded_reason'], str)
                or not item['excluded_reason'].strip() or len(item['excluded_reason']) > 4096):
            raise ValueError('Inventario fonti: motivazione di esclusione vuota o non valida')
        if 'timestamp_seconds' in item:
            t = item['timestamp_seconds']
            if type(t) not in (int, float) or not math.isfinite(t) or t < 0:
                raise ValueError('Inventario fonti: timestamp non valido')
    return inventory['items']


def coverage_report(guide, sources):
    report = {'schema_version': 1, 'status': 'unavailable', 'total': 0,
              'linked': 0, 'excluded': 0, 'issues': [],
              'scope': 'declared_instructions_only'}
    items = inventory_items(guide)
    if items is None:
        return report
    rows = {row['key']: row for lane in guide['tracks'].values() for row in lane}
    covered, observations = set(), {}
    def issue(code, message, item=None, keys=None):
        report['issues'].append({'code': code, 'message': message,
                                'item_id': item['id'] if item else None, 'step_keys': keys or []})
    for item in items:
        keys = item.get('step_keys', [])
        source = sources.get(item['source_id'])
        if not source or item['quote'] not in source['text']:
            issue('unsupported_quote', 'Citazione non presente nella fonte allegata.', item, keys)
        if 'timestamp_seconds' in item and not any(
                type(s.get('start')) in (int, float) and abs(s['start'] - item['timestamp_seconds']) <= 1
                and item['quote'] in s.get('text', '') for s in (source or {}).get('segments', [])):
            issue('unsupported_timestamp', 'Tempo non riscontrato nei segmenti della fonte.', item, keys)
        if item.get('excluded_reason'):
            if keys:
                issue('conflicting_disposition', 'Scegli collegamento oppure esclusione motivata.', item, keys)
            else:
                report['excluded'] += 1
        elif not keys:
            issue('unmapped', 'Istruzione estratta senza uno step o una esclusione motivata.', item)
        elif any(key not in rows for key in keys):
            issue('unknown_step', 'Il collegamento indica uno step inesistente.', item, keys)
        else:
            report['linked'] += 1
            covered.update(keys)
        # Exact same source occurrence only; repeated instructions at later times are legitimate.
        identity = (item['source_id'], item['quote'], item.get('timestamp_seconds'))
        if not item.get('excluded_reason'):
            if identity in observations:
                issue('duplicate_observation', 'Stesso riscontro di ' + observations[identity] + ': verifica il duplicato.', item, keys)
            else:
                observations[identity] = item['id']
    for key in rows.keys() - covered:
        issue('untracked_step', 'Step senza collegamento all’inventario: ' + key, keys=[key])
    report.update(total=len(items), status='needs_review' if report['issues'] else 'accounted')
    return report
