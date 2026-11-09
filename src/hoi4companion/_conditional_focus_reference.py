"""Compile explicit bypass/fallback notation into existing Guide v1 policies.

This resolves identities, never the meaning of an informal campaign condition.
No live reader, game-script evaluator or second runtime is introduced here.
"""
from copy import deepcopy
import hashlib
import re

from .resolution import ResolutionContext


_FALLBACK = re.compile(r'\s*([^/()]+?)\s*\(\s*bypass\s*\)\s*/\s*([^/()]+?)'
                       r'(?:\s*\(\s*if\s+([^()]+?)\s*\))?\s*', re.I)


def compile_conditional_focuses(guide, index):
    """Return resolution evidence; unresolved branches leave the row untouched.

    A source phrase such as "late with X" has no measurable predicate. Its
    namespaced fact stays unknown until an explicit qualified fact is supplied;
    it is not a checkbox and the importer does not create an observation.
    Authored when/completion/dependencies always take precedence.
    """
    result = {}
    for rows in guide['tracks'].values():
        for row in rows:
            action = row['action']
            if action['kind'] != 'select_focus' or action.get('params', {}).get('conditional_focus'):
                continue
            query = action['entity']['id']
            match = _FALLBACK.fullmatch(query)
            if not match:
                continue
            # Word's priority parser may already have moved the trailing "if"
            # note out of the entity query. Recover it only for the same names.
            if not match[3]:
                for field in ('note', 'title'):
                    full = _FALLBACK.fullmatch(row.get(field, ''))
                    if full and full[3] and all(full[i].strip().casefold() == match[i].strip().casefold() for i in (1, 2)):
                        match = full
                        break
            if not match[3]:
                continue  # A bare slash is not an implicit condition.
            context = ResolutionContext(country=guide['applies_to'].get('country'),
                                        scope=action['entity'].get('scope') or guide['applies_to'].get('tree_id'))
            branches = [index.matches(match[i].strip(), ('focus',), context, typos=True) for i in (1, 2)]
            if any(len(entries) != 1 for entries in branches):
                continue
            bypass, fallback = (entries[0] for entries in branches)
            if (bypass['id'] == fallback['id'] or bypass.get('scope') != fallback.get('scope')
                    or bypass.get('continuous') or fallback.get('continuous')):
                continue
            phrase = match[3].strip()
            token = hashlib.sha256((guide['id']+'\0'+row['key']+'\0'+phrase).encode('utf8')).hexdigest()[:24]
            guard = {'op':'fact', 'key':'guide.conditions.'+token, 'test':'eq', 'value':True}
            row.setdefault('when', guard)
            row.setdefault('completion', {'op':'any', 'args':[
                {'op':'fact', 'key':'focus.completed', 'test':'contains', 'value':entry['id']}
                for entry in (bypass, fallback)]})
            metadata = {'bypass_id':bypass['id'], 'fallback_id':fallback['id'],
                        'condition_text':phrase, 'source_query':query,
                        'condition_basis':'declared' if row['when'] != guard else 'source_text_unverifiable'}
            action.setdefault('params', {})['conditional_focus'] = metadata
            action['entity']['id'] = fallback['id']
            if fallback.get('scope'):
                action['entity']['scope'] = fallback['scope']
            row['target'] = fallback['id']
            row.setdefault('note', match[0].strip())
            result[row['key']] = deepcopy(metadata)
    return result
