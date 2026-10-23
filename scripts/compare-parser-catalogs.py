"""Compare an offline Jomini Store with a running legacy server, without live reads.

Set PYTHONPATH=src; the packaged native library is discovered automatically.
To prove old/new parity, the reference must run the pre-migration code.
Progress is isolated in a temporary directory. Never calls the live endpoints.
"""
import argparse
import json
from pathlib import Path
import tempfile
import time
from urllib.parse import urlencode
from urllib.request import urlopen
from unittest.mock import patch

from hoi4companion.api import Store


def compare(base):
    def get(path):
        with urlopen(base.rstrip('/') + path, timeout=60) as response:
            return json.load(response)

    before = get('/api/config')
    reference_catalog = get('/api/import/catalog')['entities']
    reference_technology = get('/api/technologies')
    start = time.perf_counter()
    with tempfile.TemporaryDirectory(prefix='hoi4-parser-parity-') as directory:
        with patch('hoi4companion.api.SETTINGS', Path(directory) / 'settings.json'):
            store = Store(before['settings'])
            actual_catalog = store.import_catalog()
            result = {
                'schema_version': 1,
                'native_store_seconds': time.perf_counter() - start,
                'overview_equal': {
                    field: store.overview()[field] == before[field]
                    for field in ('trees', 'excluded_trees', 'dlcs', 'mods')
                },
                'catalog_equal': actual_catalog == reference_catalog,
                'catalog_counts': [len(reference_catalog), len(actual_catalog)],
                'technology_equal': reference_technology == store.technology_catalog(),
            }
            bykey = lambda rows: {(r['kind'], r.get('scope'), r['id']): r for r in rows}
            a, b = bykey(reference_catalog), bykey(actual_catalog)
            result['catalog_removed'] = sorted(a.keys() - b.keys(), key=str)
            result['catalog_added'] = sorted(b.keys() - a.keys(), key=str)
            result['catalog_changed'] = sorted(
                (key for key in a.keys() & b.keys() if a[key] != b[key]), key=str)
            result['focus_trees'] = [
                {'id': tree['id'], 'equal': store.tree(tree['id']) == get(
                    '/api/tree?' + urlencode({'id': tree['id']}))}
                for tree in before['trees']
            ]
            result['technology_trees'] = [
                {'id': folder['id'], 'equal': store.technology_tree(folder['id']) == get(
                    '/api/technology-tree?' + urlencode({'id': folder['id']}))}
                for folder in reference_technology['folders']
            ]
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-url', required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    result = compare(args.reference_url)
    passed = (all(result['overview_equal'].values()) and result['catalog_equal']
              and result['technology_equal'] and all(
                  row['equal'] for field in ('focus_trees', 'technology_trees')
                  for row in result[field]))
    result['passed'] = passed
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(result, ensure_ascii=False))
    raise SystemExit(0 if passed else 1)
