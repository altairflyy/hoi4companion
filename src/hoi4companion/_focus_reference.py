"""Importazione e valutazione conservativa degli alberi dei focus."""
from __future__ import annotations

from dataclasses import dataclass, field, asdict
from pathlib import Path
from .clausewitz import Node
from .jomini_parser import parse_file
from .tooltips import localise, focus_tooltips


def _refs(block: Node, field_name: str) -> list[str]:
    return [value for value in block.all(field_name) if isinstance(value, str)]


def _num(source: str, default: float) -> float:
    try:
        return float(source)
    except (TypeError, ValueError):
        return default


@dataclass
class Focus:
    id: str
    name: str
    description: str
    icon: str
    x: float
    y: float
    relative_to: str
    cost: float
    prerequisites: list[list[str]]
    mutually_exclusive: list[str]
    has_conditions: bool
    has_bypass: bool
    filters: list[str]
    source: str
    position_warning: bool = False
    icon_variants: list[str] = field(default_factory=list)
    tooltip_sections: list[dict] = field(default_factory=list)
    localization_key: str = ''
    bypass_possible: bool | None = None
    sequence_effects_safe: bool | None = None
    external_completion_possible: bool | None = None
    repeatable: bool = False
    criteria: dict | None = None

    def to_json(self) -> dict:
        value = asdict(self)
        value['estimated_days'] = round(self.cost * 7)
        return value


@dataclass
class Tree:
    id: str
    source: str
    focuses: dict[str, Focus] = field(default_factory=dict)
    shared: list[str] = field(default_factory=list)
    country_selector: Node | None = None
    default: bool = False
    eligible_countries: list[str] = field(default_factory=list)
    branch_conditions: dict[str, Node] = field(default_factory=dict, repr=False)

    def to_json(self) -> dict:
        return {'id': self.id, 'source': self.source, 'count': len(self.focuses),
                'focuses': [f.to_json() for f in self.focuses.values()]}


def _build_focus(node: Node, labels: dict[str, str], source: str, *, jointly_completed=False) -> Focus | None:
    fields = node.projection if node.projection and node.projection.get('kind') == 'focus' else None
    def scalar(key):
        return (fields.get(key) or '') if fields is not None else node.scalar(key)
    focus_id = scalar('id')
    if not focus_id:
        return None
    prerequisites = fields['prerequisites'] if fields else []
    if fields is None:
        for block in node.blocks('prerequisite'):
            group = _refs(block, 'focus')
            if group:
                prerequisites.append(group)
    mutex = fields['mutually_exclusive'] if fields else [reference for block in node.blocks('mutually_exclusive') for reference in _refs(block, 'focus')]
    filt = node.first('search_filters')
    filters = [value for key, value in filt.entries if key == '' and isinstance(value, str)] if isinstance(filt, Node) else []
    variants = []
    icon = scalar('icon')
    for block in node.blocks('icon'):
        variants.extend(k for k, v in block.entries if k.startswith('GFX_'))
    text_key=node.scalar('text') or focus_id
    def resets(block):
        return any(key in ('load_focus_tree','uncomplete_national_focus','uncomplete_national_focuses')
                   or isinstance(value,Node) and resets(value) for key,value in block.entries)
    from .focus_criteria import compile_rules
    return Focus(
        id=focus_id, name=localise(labels.get(text_key, text_key), labels),
        description=localise(labels.get(text_key+'_desc', ''), labels), icon=icon,
        x=_num(scalar('x'), 0), y=_num(scalar('y'), 0),
        relative_to=scalar('relative_position_id'),
        cost=_num(scalar('cost'), 10), prerequisites=prerequisites,
        mutually_exclusive=list(dict.fromkeys(mutex)),
        has_conditions=bool(node.blocks('available') or node.blocks('allow_branch') or node.blocks('visible')),
        has_bypass=bool(node.blocks('bypass')), filters=filters, source=source,
        icon_variants=list(dict.fromkeys(variants)), tooltip_sections=focus_tooltips(node, labels),localization_key=text_key,
        bypass_possible=any(block.entries for block in node.blocks('bypass')),
        sequence_effects_safe=not any(resets(block) for block in node.blocks('completion_reward')),
        external_completion_possible=jointly_completed,
        repeatable=node.scalar('repeatable')=='yes',
        criteria=compile_rules(node),
    )


def _position_tree(tree: Tree) -> None:
    coords: dict[str, tuple[float, float]] = {}
    visiting: set[str] = set()

    def resolve(focus_id: str) -> tuple[float, float]:
        focus = tree.focuses[focus_id]
        if focus_id in coords:
            return coords[focus_id]
        if focus_id in visiting:
            focus.position_warning = True
            return focus.x, focus.y
        visiting.add(focus_id)
        ref = focus.relative_to
        if ref and ref in tree.focuses:
            px, py = resolve(ref)
            result = (px + focus.x, py + focus.y)
        else:
            if ref:
                focus.position_warning = True
            result = (focus.x, focus.y)
        visiting.remove(focus_id)
        coords[focus_id] = result
        return result

    for fid in tree.focuses:
        resolve(fid)
    for fid, (x, y) in coords.items():
        tree.focuses[fid].x, tree.focuses[fid].y = x, y


def load_trees(files: list[Path], labels: dict[str, str]) -> tuple[dict[str, Tree], list[str]]:
    trees: dict[str, Tree] = {}
    shared: dict[str, Focus] = {}
    shared_branches = {}
    warnings: list[str] = []
    for filename in files:
        try:
            root = parse_file(filename)
        except (OSError, ValueError, RecursionError) as exc:
            warnings.append(f'File ignorato: {filename.name} ({exc})')
            continue
        for key, value in root.entries:
            if key in ('shared_focus', 'joint_focus') and isinstance(value, Node):
                focus = _build_focus(value, labels, filename.name,jointly_completed=key=='joint_focus')
                if focus:
                    shared[focus.id] = focus
                    if value.blocks('allow_branch'):shared_branches[focus.id]=value.first('allow_branch')
                    else:shared_branches.pop(focus.id,None)
            elif key == 'focus_tree' and isinstance(value, Node):
                tree_id = value.scalar('id')
                if not tree_id:
                    continue
                tree = Tree(id=tree_id, source=filename.name,
                            country_selector=value.first('country'), default=value.scalar('default') == 'yes')
                for child_key, child_value in value.entries:
                    if child_key == 'focus' and isinstance(child_value, Node):
                        focus = _build_focus(child_value, labels, filename.name)
                        if focus:
                            tree.focuses[focus.id] = focus
                            if child_value.blocks('allow_branch'):tree.branch_conditions[focus.id]=child_value.first('allow_branch')
                    elif child_key in ('shared_focus', 'joint_focus') and isinstance(child_value, str):
                        tree.shared.append(child_value)
                trees[tree.id] = tree

    shared_children: dict[str, list[str]] = {}
    for fid, item in shared.items():
        for parent in {ref for group in item.prerequisites for ref in group}:
            shared_children.setdefault(parent, []).append(fid)
    for tree in trees.values():
        # A referenced root imports its descendants. Required ancestors remain
        # available, without pulling in their unrelated sibling branches.
        branch, expanded = tree.shared.copy(), set()
        while branch:
            fid = branch.pop()
            if fid in expanded:
                continue
            expanded.add(fid)
            branch.extend(shared_children.get(fid, []))
        pending = sorted(expanded, reverse=True)
        while pending:
            fid = pending.pop()
            if fid in tree.focuses:
                continue
            if fid not in shared:
                warnings.append(f'Focus condiviso non trovato: {fid} ({tree.id})')
                continue
            # Copia: le coordinate sono specifiche dell'albero.
            from dataclasses import replace
            item = replace(shared[fid])
            tree.focuses[fid] = item
            if fid in shared_branches:tree.branch_conditions[fid]=shared_branches[fid]
            for group in item.prerequisites:
                pending.extend(x for x in group if x in shared and x not in tree.focuses)
        _position_tree(tree)
    return trees, warnings


def load_continuous_focuses(files, labels):
    """Load palettes separately: their nodes do not belong in national layouts."""
    focuses, warnings = {}, []
    for path in files:
        try:
            for palette in parse_file(path).blocks('continuous_focus_palette'):
                for node in palette.blocks('focus'):
                    focus = _build_focus(node, labels, str(path))
                    if focus: focuses[focus.id] = focus
        except (OSError, ValueError, RecursionError) as exc:
            warnings.append(f'Continuous focus palette {path.name}: {exc}')
    return focuses, warnings


def evaluate(tree: Tree, completed: set[str], current: str | None = None) -> dict[str, str]:
    """Non prova a valutare i trigger complessi: stato 'da verificare'."""
    result = {}
    for fid, focus in tree.focuses.items():
        if fid in completed:
            result[fid] = 'completed'
        elif fid == current:
            result[fid] = 'active'
        elif any(x in completed for x in focus.mutually_exclusive):
            result[fid] = 'excluded'
        elif not all(any(x in completed for x in group) for group in focus.prerequisites):
            result[fid] = 'locked'
        elif focus.has_conditions:
            result[fid] = 'verify'
        else:
            result[fid] = 'ready'
    return result


def prerequisite_closure(tree: Tree, target: str) -> set[str]:
    """Tutti i *possibili* antenati (con OR non sceglie arbitrariamente un ramo)."""
    result: set[str] = set()
    stack = [target]
    while stack:
        fid = stack.pop()
        if fid in result or fid not in tree.focuses:
            continue
        result.add(fid)
        for group in tree.focuses[fid].prerequisites:
            stack.extend(group)
    return result
