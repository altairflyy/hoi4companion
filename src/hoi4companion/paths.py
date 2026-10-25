"""Resources stay with the app; user data stays outside a frozen distribution."""
import os
from pathlib import Path
import sys


def data_directory():
    override=os.environ.get('HOI4COMPANION_DATA_DIR')
    if override:return Path(override).expanduser().absolute()
    if getattr(sys,'frozen',False):
        local=os.environ.get('LOCALAPPDATA')
        return (Path(local) if local else Path.home()/'AppData'/'Local')/'HOI4Companion'
    return Path(__file__).resolve().parents[2]/'.local'


def runtime_executable():
    if getattr(sys,'frozen',False):return str(Path(sys.executable).with_name('Companion Runtime.exe'))
    return sys.executable


def module_command(module,*args):
    return [runtime_executable(),'-m',module,*map(str,args)]


def bundled_node():
    candidate=Path(__file__).parent/'_native'/'node'/'node.exe'
    return candidate if getattr(sys,'frozen',False) and candidate.is_file() else None
