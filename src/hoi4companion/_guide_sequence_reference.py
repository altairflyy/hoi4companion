"""Describe structural legacy barriers without changing a guide or a run."""


def sequence_report(guide):
    """Summarize potential implicit barriers in a normalized guide.

    These are structural counts, not a claim that barriers are currently
    blocking a run. Explicit dependencies are never inferred or altered.
    """
    policy = guide.get('dependency_policy', 'legacy')
    affected = []
    edge_count = 0
    unspecified = 0
    step_count = 0
    for track, steps in sorted(guide['tracks'].items()):
        barriers = 0
        for step in steps:
            step_count += 1
            specified = 'depends_on' in step or 'depends_on_started' in step
            if not specified:
                unspecified += 1
                if policy == 'legacy' and barriers:
                    edge_count += barriers
                    affected.append({'key': step['key'], 'track': track, 'implicit_barriers': barriers})
            if step['action']['kind'] == 'manual' and step.get('tracking') != 'reminder':
                barriers += 1
    return {'policy': policy, 'step_count': step_count,
            'unspecified_dependency_steps': unspecified,
            'implicit_barrier_steps': len(affected), 'implicit_barrier_edges': edge_count,
            'examples': affected[:10]}
