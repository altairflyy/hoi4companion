"""Read-only inventory of HOI4's saved equipment recipes, not campaign models.

File presence never proves an in-memory load, researched modules or an active
production line. No game files, presets or campaign saves are written here.
"""
import hashlib
import json
from pathlib import Path

from .clausewitz import Node
from .jomini_parser import parse_bytes


def recipes(raw):
    root = parse_bytes(raw)
    if root.scalar('version') != '0':
        raise ValueError('Unsupported saved design version')
    result = []
    for _, group in root.entries:
        if not isinstance(group, Node):
            continue
        category = group.scalar('category')
        if category not in ('land', 'naval', 'air'):
            raise ValueError('Unknown saved design category')
        for entry in group.blocks('equipment'):
            variants = entry.blocks('target_variant')
            if not entry.scalar('name') or len(variants) != 1:
                raise ValueError('Incomplete saved design')
            variant = variants[0]
            if not variant.scalar('type'):
                raise ValueError('Saved design has no equipment type')
            def fields(key):
                blocks = variant.blocks(key)
                if len(blocks) > 1:
                    raise ValueError('Repeated saved design fields')
                values = blocks[0].entries if blocks else []
                if any(not k or not isinstance(v, str) for k, v in values) or len({k for k, _ in values}) != len(values):
                    raise ValueError('Ambiguous saved design fields')
                return dict(values)
            row = {'name': entry.scalar('name'), 'category': category,
                   'equipment_id': variant.scalar('type'),
                   'modules': fields('modules'), 'upgrades': fields('upgrades')}
            result.append({**row, 'folder': entry.scalar('folder')})
    return result


def identity(row):
    return json.dumps({key: row.get(key, {} if key in ('modules', 'upgrades') else '')
                       for key in ('name', 'category', 'equipment_id', 'modules', 'upgrades')},
                      sort_keys=True, ensure_ascii=False)


class PresetInventory:
    def __init__(self):
        self.key = None
        self.rows = []
        self.digest = None

    def inspect(self, docs_dir, designs):
        """Compare complete recipes, preserving all folders and exact variants."""
        try:
            if not docs_dir:
                raise ValueError('HOI4 documents directory is not configured')
            root = Path(docs_dir).resolve(strict=True)
            path = (root / 'session_persistence/equipment_designs/equipment_designs.txt').resolve()
            if not path.is_relative_to(root):
                raise ValueError('Saved design path is outside HOI4 documents')
            stat = path.stat()
            if stat.st_size > 2_000_000:
                raise ValueError('Saved design file exceeds its inventory limit')
            key = (str(path), stat.st_mtime_ns, stat.st_size)
            if key != self.key:
                raw = path.read_bytes()
                after = path.stat()
                # Access time can change because of this read, especially on
                # Windows. Only content/file identity participates in the guard.
                if (len(raw) != stat.st_size or len(raw) > 2_000_000 or
                        (after.st_mtime_ns, after.st_size, after.st_ino) !=
                        (stat.st_mtime_ns, stat.st_size, stat.st_ino)):
                    raise ValueError('Saved design file changed during inspection')
                rows = recipes(raw)
                self.key, self.rows = key, rows
                self.digest = hashlib.sha256(raw).hexdigest()
            indexed = {}
            for row in self.rows:
                indexed.setdefault(identity(row), []).append(row['folder'])
            locations = {}
            names = {row['name'] for row in self.rows}
            for row in designs:
                folders = indexed.get(identity(row), [])
                locations[row['id']] = {'status': 'present' if folders else 'different_recipe' if row['name'] in names else 'missing',
                                        'folders': sorted(set(folders))}
            return {'status': 'file_observed', 'count': len(self.rows),
                    'distinct_recipes': len(indexed), 'sha256': self.digest,
                    'designs': locations}
        except FileNotFoundError:
            # A missing preset file is distinct from a failed/corrupt read.
            self.key = None
            return {'status': 'file_missing', 'designs': {}}
        except (OSError, ValueError, UnicodeError) as exc:
            self.key = None
            return {'status': 'unavailable', 'reason': str(exc), 'designs': {}}


def annotate(proposals, guide, inventory):
    """Attach navigation hints only. Never change matching, counts or readiness."""
    if not isinstance(inventory, dict):
        return proposals
    refs = {}
    for rows in guide.get('tracks', {}).values():
        for row in rows:
            params = row.get('action', {}).get('params', {})
            children = params.get('production_targets')
            for i, child in enumerate(children if isinstance(children, list) else [params]):
                ref = child.get('design_reference', {})
                key = row['key'] + ('.target-' + str(i + 1) if isinstance(children, list) else '')
                if ref.get('id'):
                    refs[key] = ref['id']
    for proposal in proposals:
        if proposal.get('panel') != 'production':
            continue
        value = inventory.get('designs', {}).get(refs.get(proposal['key']))
        if value:
            proposal['production_saved_design'] = value
    return proposals
