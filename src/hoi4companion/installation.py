"""Rilevamento installazione HOI4 e sovrapposizione (prudente) dei mod file."""
from __future__ import annotations

import json
import os
from pathlib import Path
from .clausewitz import parse


def default_documents() -> Path:
    documents = Path.home() / 'Documents'
    if os.name == 'nt':
        # Windows may redirect Documents to OneDrive or another drive. Read the
        # current user's location rather than carrying a developer profile path.
        try:
            import winreg
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER,
                    r'Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders') as key:
                value, _ = winreg.QueryValueEx(key, 'Personal')
                if isinstance(value, str) and value:
                    documents = Path(os.path.expandvars(value))
        except (ImportError, OSError):
            pass
    return documents / 'Paradox Interactive' / 'Hearts of Iron IV'


def detect_game() -> Path | None:
    candidates = []
    for var in ('PROGRAMFILES(X86)', 'PROGRAMFILES'):
        root = os.environ.get(var)
        if root:
            candidates.append(Path(root) / 'Steam' / 'steamapps' / 'common' / 'Hearts of Iron IV')
    candidates.extend([
        Path('C:/Steam/steamapps/common/Hearts of Iron IV'),
        Path('D:/SteamLibrary/steamapps/common/Hearts of Iron IV'),
        Path('D:/Steam/steamapps/common/Hearts of Iron IV'),
    ])
    return next((p for p in candidates if (p / 'common' / 'national_focus').is_dir()), None)


def _descriptor_value(text: str, key: str) -> str:
    return parse(text).scalar(key).strip()


def inspect_mods(docs: Path) -> dict:
    """Read the local launcher's last selected mod set, not live process proof."""
    config = docs / 'dlc_load.json'
    result = {'status': 'unavailable', 'source': str(config), 'basis': 'launcher_profile',
              'items': [], 'roots': [], 'warnings': []}
    try:
        value = json.loads(config.read_text(encoding='utf-8-sig'))
        if not isinstance(value, dict) or not isinstance(value.get('enabled_mods'), list):
            raise ValueError('enabled_mods deve essere un elenco')
        items = value['enabled_mods']
    except (ValueError, OSError) as exc:
        result['warnings'].append(f'Impossibile leggere le mod da {config}: {exc}')
        return result
    for item in items:
        entry = {'descriptor': str(item), 'name': str(item), 'path': None, 'status': 'unavailable', 'reason': ''}
        result['items'].append(entry)
        if not isinstance(item, str) or not item.strip():
            entry['reason'] = 'Riferimento mod non valido nel profilo'
            continue
        desc = docs / item.replace('\\', '/')
        try:
            root = parse(desc.read_text(encoding='utf-8-sig'))
            entry['name'] = root.scalar('name') or item
            location = root.scalar('path')
            if not location:
                entry['reason'] = ('Mod archivio ZIP non supportata' if root.scalar('archive') else 'Percorso mod assente')
                continue
            folder = Path(location.replace('\\', '/')).expanduser()
            if not folder.is_absolute():
                # Launcher paths such as mod/my_mod are relative to the HOI4
                # user directory, not docs/mod/mod. Also accept a descriptor-local
                # folder when unambiguous, for existing local mod installations.
                candidates = list(dict.fromkeys(p.resolve() for p in (docs / folder, desc.parent / folder) if p.is_dir()))
                if len(candidates) != 1:
                    entry['reason'] = ('Percorso mod relativo ambiguo' if candidates else f'Cartella mod non trovata: {location}')
                    continue
                folder = candidates[0]
            if folder.is_dir():
                entry.update(path=str(folder.resolve()), status='loaded')
            else:
                entry['reason'] = f'Cartella mod non trovata: {folder}'
        except (OSError, ValueError, RecursionError) as exc:
            entry['reason'] = f'Mod non leggibile: {exc}'
    result['warnings'] = [f"{entry['descriptor']}: {entry['reason']}" for entry in result['items'] if entry['reason']]
    for entry in result['items']:
        if entry['status'] == 'loaded':
            if entry['path'] in result['roots']:
                result['roots'].remove(entry['path'])
            result['roots'].append(entry['path'])
    result['status'] = 'partial' if result['warnings'] else 'observed'
    return result


def active_mods(docs: Path) -> tuple[list[Path], list[str]]:
    inventory = inspect_mods(docs)
    return [Path(path) for path in inventory['roots']], inventory['warnings']


def collect_files(game: Path, mod_dirs: list[Path], subdir: str, extension: str) -> tuple[list[Path], list[str]]:
    """Applica replace_path e sostituzione per percorso relativo. Non scrive sul gioco."""
    result: dict[str, Path] = {}
    warnings: list[str] = []
    roots = [game, *mod_dirs]
    for index, root in enumerate(roots):
        if index:
            descriptor = root / 'descriptor.mod'
            if descriptor.exists():
                try:
                    config = parse(descriptor.read_text(encoding='utf-8-sig', errors='replace'))
                    replacement = [p.replace('\\', '/').rstrip('/') for p in config.all('replace_path') if isinstance(p, str)]
                    if any(subdir == item or subdir.startswith(item + '/') or item.startswith(subdir + '/') for item in replacement):
                        result = {key: val for key, val in result.items()
                                  if not any(key == item or key.startswith(item + '/') for item in replacement)}
                except (OSError, ValueError):
                    warnings.append(f'Descrittore mod non valido: {descriptor}')
        base = root / subdir
        if not base.is_dir():
            continue
        for filename in sorted(base.rglob(f'*{extension}')):
            rel = (Path(subdir) / filename.relative_to(base)).as_posix().lower()
            result[rel] = filename
    return list(result.values()), warnings


def load_localisations(game: Path, mods: list[Path], language: str) -> tuple[dict[str, str], dict[str, str]]:
    """Read display overlays and an independent English reference in one pass.

    Translation mods can replace even *_l_english.yml. Preserve the game's
    English labels for existing IDs; accept mod English for newly added IDs.
    Display text retains the usual language and active-mod precedence.
    """
    import re
    languages = ['english'] if language == 'english' else ['english', language]
    result: dict[str, str] = {}
    reference: dict[str, str] = {}
    regex = re.compile(r'^\s*([^:#\s]+):\d*\s+"((?:\\.|[^"\\])*)"')
    for lang in languages:
        for root in [game, *mods]:
            english: dict[str, str] = {}
            local = root / 'localisation'
            if not local.exists():
                continue
            for f in sorted(local.rglob(f'*_l_{lang}.yml')):
                try:
                    with f.open(encoding='utf-8-sig', errors='replace') as src:
                        for line in src:
                            hit = regex.match(line)
                            if hit:
                                key = hit.group(1)
                                text = hit.group(2).replace('\\n', '\n').replace('\\"', '"')
                                result[key] = text
                                if lang == 'english':
                                    # Last file wins within each root, as for display.
                                    english[key] = text
                except OSError:
                    continue
            if lang == 'english':
                for key, text in english.items():
                    if root == game or key not in reference:
                        reference[key] = text
    return result, reference


def load_localisation(game: Path, mods: list[Path], language: str) -> dict[str, str]:
    """Compatibility API for callers that only need installed display text."""
    return load_localisations(game, mods, language)[0]
