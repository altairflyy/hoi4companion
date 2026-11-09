"""Read a Word guide or collection without extracting files or executing content."""
from __future__ import annotations

from pathlib import Path
from contextlib import contextmanager
import tempfile
from zipfile import BadZipFile, ZipFile


W = '{http://schemas.openxmlformats.org/wordprocessingml/2006/main}'
MAX_DOC = 12_000_000
MAX_XML = 8_000_000
MAX_ENTRIES = 5000


def _safe_entry(name):
    return (isinstance(name, str) and len(name) <= 1200 and '\\' not in name
            and not name.startswith('/') and all(part not in ('', '.', '..') for part in name.split('/')))


def _read(z, info, limit):
    if info.file_size > limit or info.flag_bits & 1:
        raise ValueError('Documento troppo grande o protetto da password')
    try:
        with z.open(info) as stream:
            data = stream.read(limit + 1)
    except (BadZipFile, RuntimeError, EOFError, NotImplementedError) as exc:
        raise ValueError('Archivio danneggiato o compressione non supportata') from exc
    if len(data) > limit:
        raise ValueError('Documento troppo grande')
    return data


def _archive(stream):
    try:
        z = ZipFile(stream)
        if len(z.infolist()) > MAX_ENTRIES:
            z.close(); raise ValueError('Archivio con troppi elementi')
        return z
    except (BadZipFile, OSError) as exc:
        raise ValueError('Documento Word o archivio ZIP non valido') from exc


@contextmanager
def _native_source(stream):
    candidate=getattr(stream,'name',None)
    if isinstance(candidate,(str,bytes)):
        path=Path(candidate)
        if path.is_file():
            yield path
            return
    temporary=None
    try:
        stream.seek(0)
        with tempfile.NamedTemporaryFile(suffix='.hoi4-source.zip',delete=False) as target:
            temporary=Path(target.name);remaining=200_000_001
            while remaining:
                chunk=stream.read(min(1024*1024,remaining))
                if not chunk:break
                target.write(chunk);remaining-=len(chunk)
            if remaining==0:raise ValueError('File vuoto o superiore a 200 MB')
        yield temporary
    finally:
        if temporary is not None:temporary.unlink(missing_ok=True)


def list_documents(stream):
    from .backend_core import _request
    with _native_source(stream) as path:
        return _request(0,'word_documents',path=str(path))


def prepare_document(stream, entry=None, context_entry=None):
    from .backend_core import _request
    from .research_upgrades import guide_language
    with _native_source(stream) as path:
        return _request(0,'word_prepare',path=str(path),entry=entry,context=context_entry,vocabulary=guide_language())
