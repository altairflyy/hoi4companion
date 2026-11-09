"""Bounded, deterministic identity constraints over source slots, never live state.

Complete coverage is an explicit source assertion, not a property inferred from
the number of recognized names. Possible bypasses over-approximate solutions;
they can weaken a proof but can never manufacture one.
"""
from copy import deepcopy

from .resolution import ResolutionContext, catalog_index


CLASSES = ('direct', 'reconstructed', 'probable', 'unresolved')


def validate_metadata(guide):
    policy = guide.get('focus_sequence')
    if policy is not None:
        if (not isinstance(policy, dict) or set(policy) - {'schema_version', 'coverage', 'initial_focuses'}
                or type(policy.get('schema_version')) is not int or policy['schema_version'] != 1
                or policy.get('coverage') not in ('partial', 'complete')):
            raise ValueError('Invalid focus sequence policy')
        initial = policy.get('initial_focuses', [])
        if (policy['coverage'] == 'complete' and 'initial_focuses' not in policy
                or not isinstance(initial, list) or len(initial) > 4096
                or any(not isinstance(x, str) or not x or len(x) > 512 for x in initial)
                or len(set(initial)) != len(initial)):
            raise ValueError('Invalid initial focus list')
    positions = set()
    from .contracts import validate_entity
    for rows in guide['tracks'].values():
        previous = 0
        for row in rows:
            if 'focus_resolution' not in row:
                continue
            r = row['focus_resolution']
            if (row['action']['kind'] != 'select_focus' or not isinstance(r, dict)
                    or set(r) - {'schema_version', 'position', 'query', 'classification', 'entity', 'locked'}
                    or type(r.get('schema_version')) is not int or r['schema_version'] != 1
                    or type(r.get('position')) is not int or not 1 <= r['position'] <= 4096
                    or r['position'] in positions or r['position'] <= previous
                    or not isinstance(r.get('query'), str) or not 0 < len(r['query']) <= 2048
                    or r.get('classification') not in CLASSES
                    or 'locked' in r and type(r['locked']) is not bool):
                raise ValueError('Invalid focus source position or resolution provenance')
            validate_entity(r.get('entity'))
            if r['entity']['kind'] != 'focus':
                raise ValueError('Invalid focus provenance entity')
            positions.add(r['position']); previous = r['position']


def source_guide(guide):
    from .backend_core import _request
    return _request(0,'focus_source_guide',guide=guide)


def reconstruct(guide,catalog,named,*,work_limit=2_000_000):
    from .backend_core import _request
    index=catalog_index(catalog)
    result=_request(0,'focus_reconstruct',guide=guide,named=named,work_limit=work_limit,**index.native_import_args())
    return result['forced'],result['report'],result['errors']
