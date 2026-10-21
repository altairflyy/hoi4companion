"""Rilevamento installazione HOI4 e sovrapposizione (prudente) dei mod file."""
from __future__ import annotations

import json
import os
from pathlib import Path
from .clausewitz import parse


def default_documents() -> Path:
    return Path.home() / 'Documents' / 'Paradox Interactive' / 'Hearts of Iron IV'


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


def active_mods(docs: Path) -> tuple[list[Path], list[str]]:
    config = docs / 'dlc_load.json'
    if not config.is_file():
        return [], []
    try:
        items = json.loads(config.read_text(encoding='utf-8-sig')).get('enabled_mods', [])
    except (ValueError, OSError) as exc:
        return [], [f'Impossibile leggere dlc_load.json: {exc}']
    dirs: list[Path] = []
    warnings: list[str] = []
    for item in items:
        item = str(item).replace('\\', '/')
        desc = docs / item
        if not desc.is_file():
            warnings.append(f'Descrittore mod non trovato: {item}')
            continue
        try:
            root = parse(desc.read_text(encoding='utf-8-sig', errors='replace'))
            location = root.scalar('path')
            if not location:
                if root.scalar('archive'):
                    warnings.append(f'Mod archivio ZIP non supportata: {item}')
                continue
            folder = Path(location)
            if not folder.is_absolute():
                folder = (desc.parent / folder).resolve()
            if folder.is_dir():
                dirs.append(folder)
            else:
                warnings.append(f'Cartella mod non trovata: {folder}')
        except (OSError, ValueError) as exc:
            warnings.append(f'Mod non leggibile ({item}): {exc}')
    return dirs, warnings


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


def load_localisation(game: Path, mods: list[Path], language: str) -> dict[str, str]:
    """Lettura minimamente invasiva dei valori testuali standard HOI4."""
    import re
    languages = ['english'] if language == 'english' else ['english', language]
    result: dict[str, str] = {}
    regex = re.compile(r'^\s*([^:#\s]+):\d*\s+"((?:\\.|[^"\\])*)"')
    for lang in languages:
        for root in [game, *mods]:
            local = root / 'localisation'
            if not local.exists():
                continue
            for f in sorted(local.rglob(f'*_l_{lang}.yml')):
                try:
                    with f.open(encoding='utf-8-sig', errors='replace') as src:
                        for line in src:
                            hit = regex.match(line)
                            if hit:
                                result[hit.group(1)] = hit.group(2).replace('\\n', '\n').replace('\\"', '"')
                except OSError:
                    continue
    return result
