"""Source positions for Word action blocks; presentation, never runtime gates."""
import re


def annotate(guide):
    """Recover legacy blocks from their original quotes without rewriting a run.

    Word keys preserve the original cell order, including gaps occupied by the
    other columns. Only full quotes found in Actions columns qualify. Different
    sources or ambiguous quotes are not merged based on similar wording.
    """
    groups = {}
    for rows in guide['tracks'].values():
        for row in rows:
            if row['action']['kind'] in ('select_focus', 'research', 'free'):
                continue
            hit = re.fullmatch(r'(word-(\d+))(?:(?:--operation-|--note-)\d+)*', row['key'])
            if hit:
                groups.setdefault((int(hit[2]), hit[1]), []).append(row)
    changed = []
    for source in guide.get('sources', []):
        if source.get('type') != 'file':
            continue
        columns = re.findall(r'\[Column 1\]\n([\s\S]*?)(?=\[Column [23]\]|\Z)', source.get('text', ''))
        if not columns:
            continue
        ordinal = 0
        for (_, key), rows in sorted(groups.items()):
            quotes = {e.get('quote') for row in rows for e in row.get('evidence', [])
                      if e.get('source_id') == source['id'] and e.get('quote')}
            if len(quotes) != 1:
                continue
            quote = next(iter(quotes))
            if not any(re.search(r'(?:^|\n)' + re.escape(quote) + r'(?=\n|$)', c) for c in columns):
                continue
            ordinal += 1
            position = {'source_id': source['id'], 'key': key, 'ordinal': ordinal}
            for row in rows:
                params = row['action'].setdefault('params', {})
                if 'source_action' in params:
                    continue
                params['source_action'] = position.copy()
                changed.append({'key': row['key'], 'actions': 0, 'step_keys': [row['key']], 'unverifiable_timing': False})
    return changed
