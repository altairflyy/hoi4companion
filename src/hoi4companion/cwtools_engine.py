"""Full upstream CWTools semantic engine, invoked by the Rust native facade.

The effective manifest reuses the launcher's ordered overlays. Static diagnostics
never provide observations about the running campaign or guide completion.
"""
from collections import Counter
import ctypes
import hashlib
import json
from pathlib import Path
import sys

from .installation import collect_files
from .jomini_parser import _library, _selected_library

ROOT = Path(__file__).parent
HOST_ROOT = ROOT/'_native'/'cwtools'
RULES = ROOT/'rules'/'cwtools'
FOLDERS = ('common','country_metadata','events','gfx','interface','localisation',
           'history','map','music','portraits','sound')
EXTENSIONS = {'.txt','.gui','.gfx','.sfx','.asset','.map','.yml','.csv',
              '.dds','.tga','.shader','.lua','.png','.mesh','.ttf','.otf','.wav'}
ENGINE_REVISION = 'b377453dee803f9258be92cfc49896d09039702d'


def installation_files(game, mods):
    files={};warnings=[]
    for folder in FOLDERS:
        selected, issues=collect_files(game,mods,folder,'')
        warnings.extend(issues)
        for file in selected:
            if file.is_file() and file.suffix.lower() in EXTENSIONS:
                relative=next(file.relative_to(base).as_posix()
                              for base in reversed([game,*mods]) if file.is_relative_to(base))
                files[relative]=str(file.absolute())
    return dict(sorted(files.items())),warnings


def _verify(directory):
    manifest=json.loads((directory/'provenance.json').read_text(encoding='utf-8'))
    for file,digest in manifest['files'].items():
        target=directory/file
        if not target.is_file() or hashlib.sha256(target.read_bytes()).hexdigest()!=digest:
            raise RuntimeError('CWTools: file mancante o modificato: '+file)
    return manifest


def analyze(files, *, rules_dir=RULES, host_dir=None):
    directory=Path(host_dir) if host_dir else HOST_ROOT
    manifest=_verify(directory)
    if manifest.get('revision')!=ENGINE_REVISION:
        raise RuntimeError('Versione host CWTools non supportata')
    _verify(rules_dir)
    host=str((directory/('Companion.Cwtools.exe' if sys.platform=='win32' else 'Companion.Cwtools')).resolve())
    parser_path=_selected_library()
    request=json.dumps({'schema_version':1,'rules_dir':str(rules_dir.resolve()),
                        'parser_library':str(Path(parser_path).resolve()),'files':files}).encode()
    if len(request)>16_000_000:raise ValueError('Manifesto CWTools oltre 16 MB')
    dll=_library(parser_path)
    try:function=dll.hoi4_cwtools_analyze
    except AttributeError as exc:raise RuntimeError('Ricompila il parser Rust: collegamento CWTools assente') from exc
    function.argtypes=[ctypes.c_void_p,ctypes.c_size_t,ctypes.c_void_p,ctypes.c_size_t]
    function.restype=ctypes.c_void_p
    host_bytes=host.encode();host_buffer=ctypes.create_string_buffer(host_bytes)
    request_buffer=ctypes.create_string_buffer(request)
    pointer=function(host_buffer,len(host_bytes),request_buffer,len(request))
    if not pointer:raise RuntimeError('Nessuna risposta dal collegamento CWTools')
    try:result=json.loads(ctypes.string_at(pointer))
    finally:dll.hoi4_parser_free(pointer)
    if result.get('error'):raise RuntimeError(result['error'])
    if result.get('schema_version')!=1 or result.get('engine')!='cwtools' or result.get('engine_revision')!=ENGINE_REVISION:
        raise RuntimeError('Contratto motore CWTools non supportato')
    if result.get('parser_compatibility')!=1:raise RuntimeError('Collegamento CWTools/Jomini precedente: ricompila i componenti nativi')
    if result.get('reviewed_recovery')!=1:raise RuntimeError('Host CWTools precedente: ricompila il supporto al recupero verificato')
    return result


def normalize_report(raw, files, warnings, limit=2000):
    rows=[];counts=Counter();severity=Counter()
    ignored=[e for e in raw['parser_errors'] if e.get('classification')=='rejected_by_both']
    for item in raw['diagnostics']:
        row=dict(item);counts['total']+=1;counts[row['category']]+=1;severity[row['severity']]+=1
        row.update(source='static_analysis',runtime_evaluated=False,rule=row['code'],value='',
                   path=f"{row['line']}:{row['column']}",
                   position={key:row[key] for key in ('line','column','end_line','end_column')})
        if len(rows)<limit:rows.append(row)
    counts.update(severity)
    if 'diagnostic_counts' in raw:counts=Counter(raw['diagnostic_counts'])
    provenance=json.loads((RULES/'provenance.json').read_text(encoding='utf-8'))
    return {'schema_version':1,'source':'static_analysis','runtime_evaluated':False,
            'status':'partial' if raw['parser_errors'] or warnings or raw.get('source_recoveries') else 'analyzed',
            'rules':{'repository':provenance['repository'],'revision':provenance['revision'],
                     'adapter':'cwtools-engine-v1','engine':'cwtools','engine_revision':raw['engine_revision'],
                     'parser_compatibility':raw.get('parser_compatibility',0),
                     'reviewed_recovery':raw.get('reviewed_recovery',0),
                     'game_version_compatibility':'not_certified','load_errors':[]},
            'coverage':{'documents':raw['documents'],'entities':raw['entities'],
                        'recognized':None,'unsupported':None,'unknown_scope':None,'unhandled':{},
                        'metrics_available':False,'type_names':raw['type_names'],
                        'localisation_sources':raw.get('localisation_sources',[]),
                        'parser_errors':len(raw['parser_errors']),
                        'ignored_parser_errors':len(ignored),
                        'recovered_sources':len(raw.get('source_recoveries',[]))},
            'summary':dict(counts),'diagnostics':rows,'truncated':counts['total']>len(rows),
            'parser_error_details':raw['parser_errors'],
            'ignored_source_errors':ignored,
            'source_recoveries':raw.get('source_recoveries',[]),
            'input_adjustments':raw.get('input_adjustments',[]),
            'non_script_files':raw.get('non_script_files',[]),
            'file_warnings':[*warnings,*[f"{e['file']}:{e['line']}: "
                + e['message'] for e in raw['parser_errors']
                if e.get('classification')!='rejected_by_both']],
            'sources':files,'elapsed_ms':raw.get('elapsed_ms')}


def analyze_installation(game,mods):
    files,warnings=installation_files(game,mods)
    return normalize_report(analyze(files),files,warnings)
