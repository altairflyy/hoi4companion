"""Fully resolved compound commands, preserving explicit sequence milestones."""
import re

from ._informal_guides_reference import explicit_action
from ._informal_targets_reference import named_command
from .political_import import bind_political
from .resolution import ResolutionContext


def sequence(text, catalog, country):
    """Do not split prose, alternatives, or partially recognized commands.

    'Then' declares a dependency. '&' declares several instructions but no
    completion barrier. Existing runtime ordering of political purchases still
    applies. An explicit 'start project' releases the next step on start.
    """
    parts = re.split(r'\s*(,?\s+then\s+|\s+&\s+)\s*', text, flags=re.I)
    if len(parts) < 3 or len(parts) > 31:
        return None
    result = []
    for index in range(0, len(parts), 2):
        command = parts[index].strip()
        law = explicit_action(command)
        item = None
        if law and law['type'] == 'law':
            probe = {'action': {'kind': 'manual', 'params': {'category': 'politics', 'law_name': law['target']}}}
            if bind_political(probe, catalog, country) == 'politics_live_law':
                item = {'params': probe['action']['params'], 'policies': {}, 'milestone': 'completion'}
                if 'political_power_cost' in law:
                    item['policies']['political_power_cost'] = law['political_power_cost']
        project = re.fullmatch(r'(start|complete)\s+(?:special\s+)?project\s*:\s*(.+)', command, re.I)
        if item is None and project:
            matches = catalog.matches(project[2], ('project',), ResolutionContext(country=country))
            if len(matches) == 1:
                item = {'params': {'category': 'projects', 'project_id': matches[0]['id'], 'project_name': matches[0]['name']},
                        'policies': {}, 'milestone': 'start' if project[1].lower() == 'start' else 'completion'}
        if item is None:
            named = named_command(command, catalog, country)
            # Inline timing requires its own full compiler, never discard it.
            if named and not named['timing']:
                item = {'params': named['params'], 'policies': named['policies'], 'milestone': 'completion'}
        if item is None:
            return None
        item.update(text=command, after_previous=index > 0 and 'then' in parts[index-1].lower())
        result.append(item)
    return result
