"""Required Jomini C ABI bridge. No handwritten parser or silent fallback."""
import ctypes
from functools import lru_cache
from importlib.machinery import EXTENSION_SUFFIXES
import json
import os
from pathlib import Path
import sys


class ParserUnavailableError(RuntimeError):
    """A deployment failure, not a malformed game script to skip in a catalog."""


@lru_cache(maxsize=1)
def _package_library():
    directory = Path(__file__).resolve().parent / '_native'
    suffix = '.dll' if sys.platform == 'win32' else '.dylib' if sys.platform == 'darwin' else '.so'
    for extension in dict.fromkeys([*EXTENSION_SUFFIXES, suffix]):
        candidate = directory / ('hoi4companion_parser' + extension)
        if candidate.is_file():
            return candidate
    raise ParserUnavailableError(
        'Parser Jomini non installato. Installa il pacchetto binario con pip oppure, '
        'dal sorgente, esegui scripts/build-parser.ps1 (Windows) o pip install -e . '
        'con Rust e i build tools disponibili.')


def _selected_library():
    if os.environ.get('HOI4_PARSER', 'jomini') != 'jomini':
        raise ParserUnavailableError('HOI4_PARSER: Jomini è ora il solo parser del programma; '
                                     'rimuovi la selezione del vecchio backend.')
    override = os.environ.get('HOI4_JOMINI_LIBRARY')
    return str(Path(override).expanduser().absolute() if override else _package_library())


@lru_cache(maxsize=4)
def _library(path):
    try:
        dll = ctypes.CDLL(str(Path(path).resolve(strict=True)))
        return _configure(dll)
    except (OSError, AttributeError, ValueError) as exc:
        raise ParserUnavailableError(f'Parser Jomini non disponibile: {exc}') from exc


def _configure(dll):
    dll.hoi4_parser_schema_version.argtypes = []
    dll.hoi4_parser_schema_version.restype = ctypes.c_uint32
    if dll.hoi4_parser_schema_version() != 1:
        raise ValueError('Versione ABI del parser Jomini non supportata')
    dll.hoi4_parser_parse.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    dll.hoi4_parser_parse.restype = ctypes.c_void_p
    dll.hoi4_parser_process.argtypes = [ctypes.c_void_p,ctypes.c_size_t,ctypes.c_uint32,ctypes.c_uint32]
    dll.hoi4_parser_process.restype = ctypes.c_void_p
    dll.hoi4_parser_free.argtypes = [ctypes.c_void_p]
    dll.hoi4_parser_free.restype = None
    return dll


def _response(dll, data, encoding=1, mode=0):
    buffer = ctypes.create_string_buffer(data)
    result = dll.hoi4_parser_process(buffer, len(data), encoding, mode)
    if not result:
        raise ValueError('Il parser Jomini non ha restituito un risultato')
    try:
        value = json.loads(ctypes.string_at(result))
    finally:
        dll.hoi4_parser_free(result)
    if not isinstance(value, dict) or value.get('schema_version') != 1:
        raise ValueError('Contratto dati del parser Jomini non supportato')
    if 'error' in value:
        raise ValueError(value['error'])
    if value.get('engine') != 'jomini' or value.get('engine_version') != '0.37.2':
        raise ValueError('Versione motore Jomini non supportata')
    if (value.get('document_model') != 'TextTape' or value.get('typed_models') != 2
            or value.get('capabilities') != ['windows-1252','jomini-json','typed-focus-relations']):
        raise ValueError('Libreria Jomini precedente: ricompila il parser con modelli v2, codifiche e JSON')
    return value


@lru_cache(maxsize=4)
def _verified_info(path):
    try:
        value = _response(_library(path), b'')
        if value.get('node') != {'entries': []}:
            raise ValueError('Risposta iniziale del parser non valida')
    except ValueError as exc:
        raise ParserUnavailableError(f'Controllo iniziale Jomini fallito: {exc}') from exc
    return {'engine': value['engine'], 'engine_version': value['engine_version'],
            'schema_version': value['schema_version'], 'document_model':value['document_model'],
            'typed_models':value['typed_models'],'capabilities':value['capabilities']}


def backend_info():
    """Validate the actual library before loading any catalog; safe for /api/config."""
    return _verified_info(_selected_library()).copy()


def parse(source):
    return parse_bytes(source.encode('utf-8'),encoding='utf-8')


def _process(data, encoding='auto', mode=0):
    encodings={'auto':0,'utf-8':1,'windows-1252':2}
    if encoding not in encodings:
        raise ValueError('Codifica non supportata: auto, utf-8, windows-1252')
    if not isinstance(data,bytes):
        raise TypeError('La sorgente deve essere bytes')
    if len(data)>80_000_000:
        raise ValueError('Script oltre 80 MB')
    path=_selected_library()
    _verified_info(path)
    return _response(_library(path),data,encodings[encoding],mode)


def parse_file(path, *, encoding='auto'):
    with Path(path).open('rb') as stream:
        data=stream.read(80_000_001)
    return parse_bytes(data,encoding=encoding)


def export_json(source, *, encoding='auto', duplicate_keys='key_value_pairs'):
    modes={'key_value_pairs':1,'group':2,'preserve':3}
    if duplicate_keys not in modes:
        raise ValueError('Modalità chiavi duplicate non supportata')
    if isinstance(source,str):
        if encoding not in ('auto','utf-8'):
            raise ValueError('La codifica di una stringa già decodificata è utf-8')
        source=source.encode('utf-8');encoding='utf-8'
    return _process(source,encoding,modes[duplicate_keys])


def parse_bytes(data, *, encoding='auto'):
    from .clausewitz import Node
    value = _process(data,encoding)

    def convert(raw, depth=0):
        if depth > 180 or not isinstance(raw, dict) or not {'entries'} <= set(raw) or set(raw) - {'entries','projection'} or not isinstance(raw['entries'], list):
            raise ValueError('AST Jomini non valido')
        node = Node()
        for pair in raw['entries']:
            if not isinstance(pair, list) or len(pair) != 2 or not isinstance(pair[0], str):
                raise ValueError('Voce AST Jomini non valida')
            key, item = pair
            node.entries.append((key, item if isinstance(item, str) else convert(item, depth + 1)))
        projection = raw.get('projection')
        if projection is not None:
            fields = {'focus':{'id','x','y','cost','relative_position_id','icon','prerequisites','mutually_exclusive'},
                      'technology':{'research_cost','start_year','is_special_project_tech'}}
            if (not isinstance(projection,dict) or projection.get('kind') not in fields
                    or set(projection) != fields[projection['kind']] | {'kind'}
                    or any(v is not None and not isinstance(v,str) for k,v in projection.items() if k not in ('kind','prerequisites','mutually_exclusive'))):
                raise ValueError('Proiezione tipizzata Jomini non valida')
            if projection['kind']=='focus':
                refs=projection['prerequisites'];mutex=projection['mutually_exclusive']
                if (not isinstance(refs,list) or any(not isinstance(group,list) or not all(isinstance(x,str) for x in group) for group in refs)
                        or not isinstance(mutex,list) or not all(isinstance(x,str) for x in mutex)):
                    raise ValueError('Riferimenti focus tipizzati non validi')
            node.projection=projection
        return node
    node=convert(value.get('node'))
    node.source_encoding=value['source_encoding']
    return node
