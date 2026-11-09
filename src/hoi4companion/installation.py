"""Rilevamento installazione HOI4 e sovrapposizione (prudente) dei mod file."""
from __future__ import annotations

import json
import os
from pathlib import Path
from .clausewitz import parse
from .jomini_parser import parse_file


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
    from .backend_core import _request
    return _request(0,'installation_mods',docs=str(docs))


def active_mods(docs: Path) -> tuple[list[Path], list[str]]:
    inventory = inspect_mods(docs)
    return [Path(path) for path in inventory['roots']], inventory['warnings']


def collect_files(game: Path, mod_dirs: list[Path], subdir: str, extension: str) -> tuple[list[Path], list[str]]:
    from .backend_core import _request
    value=_request(0,'installation_files',game=str(game),mods=list(map(str,mod_dirs)),subdir=subdir,extension=extension)
    return list(map(Path,value['files'])),value['warnings']


def load_localisations(game: Path, mods: list[Path], language: str) -> tuple[dict[str, str], dict[str, str]]:
    from .backend_core import _request
    return tuple(_request(0,'installation_localisations',game=str(game),mods=list(map(str,mods)),language=language))


def load_localisation(game: Path, mods: list[Path], language: str) -> dict[str, str]:
    """Compatibility API for callers that only need installed display text."""
    return load_localisations(game, mods, language)[0]
