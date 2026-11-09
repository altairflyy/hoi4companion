"""Read a Word guide or collection without extracting files or executing content."""
from __future__ import annotations

import hashlib
import io
import re
from pathlib import PurePosixPath
from xml.etree import ElementTree as ET
from zipfile import BadZipFile, ZipFile

from .import_sources import MAX_TEXT, acquire_source
from ._informal_guides_reference import abbreviation_hint, explicit_action

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


def list_documents(stream):
    with _archive(stream) as z:
        names = [i.filename for i in z.infolist() if not i.is_dir()]
        if 'word/document.xml' in names:
            return {'documents': [], 'single_document': True}
        docs = [i.filename for i in z.infolist() if not i.is_dir() and _safe_entry(i.filename)
                and i.filename.lower().endswith('.docx') and i.file_size <= MAX_DOC]
        if len(set(docs)) != len(docs):
            raise ValueError('Archivio con documenti dal nome duplicato')
        return {'documents': sorted(docs, key=str.casefold), 'single_document': False}


def _paragraph(node):
    out = []
    for child in node.iter():
        if child.tag == W+'t': out.append(child.text or '')
        elif child.tag == W+'tab': out.append('\t')
        elif child.tag in (W+'br', W+'cr'): out.append('\n')
    return ''.join(out).strip()


def _priority_target(line):
    """Remove editorial wrappers from the lookup, never from source evidence.

    Balanced parentheses can themselves contain PP costs. Do not treat a slash
    or an instruction inside the remaining name as a choice of native entity.
    """
    target = line.strip().rstrip('*').strip()
    while target.endswith(')'):
        depth = 0
        for index in range(len(target)-1, -1, -1):
            if target[index] == ')': depth += 1
            elif target[index] == '(': depth -= 1
            if depth == 0:
                target = target[:index].strip().rstrip('*').strip()
                break
        else: break
    if target.startswith('('):
        end = target.find(')')
        if end > 0: target = target[end+1:].strip()
    target = re.sub(r'^(?:ASAP\s*:\s*|On\s+[^:]+:\s*)', '', target, flags=re.I)
    return target.strip()


def _word_content(data):
    with _archive(io.BytesIO(data)) as z:
        matches = [i for i in z.infolist() if i.filename == 'word/document.xml']
        if len(matches) != 1: raise ValueError('Contenuto Word mancante o duplicato')
        info = matches[0]
        raw = _read(z, info, MAX_XML)
        if b'<!DOCTYPE' in raw.upper() or b'<!ENTITY' in raw.upper():
            raise ValueError('XML Word non supportato')
        try: root = ET.fromstring(raw)
        except ET.ParseError as exc: raise ValueError('Documento Word danneggiato') from exc
        body = root.find(W+'body')
        if body is None: raise ValueError('Contenuto Word mancante')
        media = sum(1 for i in z.infolist() if i.filename.startswith('word/media/') and not i.is_dir())
        links = []
        rels = [i for i in z.infolist() if i.filename == 'word/_rels/document.xml.rels']
        if len(rels) > 1: raise ValueError('Riferimenti Word duplicati')
        if rels:
            rel_data = _read(z, rels[0], MAX_XML)
            if b'<!DOCTYPE' in rel_data.upper() or b'<!ENTITY' in rel_data.upper(): raise ValueError('Riferimenti Word non supportati')
            try: relations = ET.fromstring(rel_data)
            except ET.ParseError as exc: raise ValueError('Riferimenti Word danneggiati') from exc
            for rel in relations:
                target = rel.get('Target', '')
                if (rel.get('TargetMode') == 'External' and rel.get('Type', '').endswith('/hyperlink')
                        and target.startswith(('https://', 'http://')) and len(target) <= 4096):
                    links.append(target)
    lines, groups = [], []
    for block in body:
        if block.tag == W+'p':
            text = _paragraph(block)
            if text: lines.append(text)
        elif block.tag == W+'tbl':
            rows = []
            for row in block.findall(W+'tr'):
                cells = [[text for p in cell.iter(W+'p') if (text := _paragraph(p))]
                         for cell in row.findall(W+'tc')]
                rows.append(cells)
            if not rows: continue
            headings = [' '.join(cell).strip().upper().rstrip('*') for cell in rows[0]]
            is_priority = len(headings) == 3 and headings[0] == 'ACTIONS' and headings[1] == 'RESEARCH PRIORITY' and headings[2] == 'FOCUS PRIORITY'
            for row in rows:
                lines.append('[Table row]')
                for index, cell in enumerate(row):
                    lines.append('[Column '+str(index+1)+']')
                    lines.extend(cell)
            if is_priority:
                groups.extend(row for row in rows[1:] if len(row) == 3)
    if links: lines.extend(['[Document links]', *dict.fromkeys(links)])
    text = '\n'.join(lines)
    if not text or len(text) > MAX_TEXT: raise ValueError('Testo Word vuoto o superiore al limite di importazione')
    return text, groups, media


def prepare_document(stream, entry=None, context_entry=None):
    if context_entry and context_entry == entry:
        raise ValueError('La fonte di contesto deve essere diversa dalla guida')
    with _archive(stream) as z:
        names = [i.filename for i in z.infolist() if not i.is_dir()]
        if 'word/document.xml' in names and entry is None:
            stream.seek(0); data = stream.read(MAX_DOC + 1); title = 'Word guide.docx'
            if len(data) > MAX_DOC: raise ValueError('Documento Word troppo grande')
        else:
            if not _safe_entry(entry) or not entry.lower().endswith('.docx'):
                raise ValueError('Seleziona un documento Word presente nell’archivio')
            matches = [i for i in z.infolist() if i.filename == entry and not i.is_dir()]
            if len(matches) != 1: raise ValueError('Documento assente o duplicato nell’archivio')
            data = _read(z, matches[0], MAX_DOC); title = PurePosixPath(entry).name
    text, groups, media = _word_content(data)
    source = acquire_source(text=text)
    source.update(type='file', title=title)
    source['warnings'] = ['Il testo è estratto dal documento Word; immagini incorporate e allegati separati non sono interpretati automaticamente.']
    steps = []
    def add(typ, line, **extra):
        step = {'key': 'word-'+str(len(steps)+1).zfill(4), 'type': typ,
                'evidence': [{'source_id': source['id'], 'basis': 'explicit', 'quote': line}], **extra}
        steps.append(step)
    action_ordinal = 0
    for actions, research, focus in groups:
        for line in actions:
            action_ordinal += 1
            hint = abbreviation_hint(line)
            action = explicit_action(line)
            typ = action.pop('type') if action else 'reminder'
            add(typ, line, title=line[:180], instructions=[line], **(action or {}), **({'note':hint} if hint else {}))
            steps[-1]['details'] = {'source_action': {'source_id': source['id'],
                'key': steps[-1]['key'], 'ordinal': action_ordinal}}
        for lane, values in (('research',research),('focus',focus)):
            for line in values:
                target = _priority_target(line)
                choice=target or line.strip().strip('()').strip()
                if re.match(r'(?i)^free(?:\s*$|\s*[/–—:-])', choice) or re.fullmatch(r'(?i)[\w -]+\s*/\s*free',choice) or re.fullmatch(r'(?i)future focuses are free/optional',choice):
                    add('free', line, lane=lane, title=line[:180], note=line); continue
                if (re.fullmatch(r'(?i)no focus.*|no research.*|nothing|(?:my )?suggestions?\s*:?|suggested\s*:|air techs|industry upgrades', choice)
                        or re.search(r'(?i)/\s*feel free to pick another one or none\s*$',choice)):
                    add('reminder', line, title=line[:180], instructions=[line]); continue
                if not target:
                    if lane == 'focus':
                        # An unreadable non-empty focus cell still occupies a
                        # source slot; do not turn a hole into an editorial note.
                        add('focus',line,target=line,title=line[:180],note=line)
                    else:
                        add('reminder', line, title=line[:180], instructions=[line])
                    continue
                typ = 'special_project' if lane == 'research' and re.search(r'\bspecial project\b', line, re.I) else lane
                if typ == 'special_project':
                    target = re.sub(r'^\s*special\s+project\s*:\s*', '', target, flags=re.I)
                    target = re.sub(r'^\s*ASAP\s*:\s*', '', target, flags=re.I)
                    target = re.sub(r'\s*\(\s*special\s+project\s*,\s*(?:get|assign|hire|recruit)\s+(?:an?\s+)?scientist\s*\)', '', target, flags=re.I)
                    target = re.sub(r'\s*\(\s*special\s+project\s*\)', '', target, flags=re.I)
                    target = re.sub(r'\s*\(\s*(?:get|assign|hire|recruit)\s+(?:an?\s+)?scientist(?:\s*\(\s*\d+\s*PP\s*\))?\s*\)', '', target, flags=re.I)
                    target = re.sub(r'\s*\(\s*(?:optional|if\s+(?:possible|available)|(?:might|may|can)\s+be\s+later)\s*\)\s*$', '', target, flags=re.I).strip()
                    target = re.sub(r'\s*\(\s*(?:get|assign|hire|recruit)\s+(?:an?\s+)?scientist\s*,\s*optional\s*\)\s*$', '', target, flags=re.I).strip()
                    target = re.sub(r'^flame\s+tanks?$', 'Flamethrower Tank', target, flags=re.I)
                    if not target:
                        add('reminder', line, title=line[:180], instructions=[line]); continue
                add(typ, line, target=target, title=line[:180], **({'note':line} if target != line else {}))
    if not steps:
        add('reminder', text[:min(2000,len(text))], title='Review source document', instructions=['Read the source document; no recognized priority table was found.'])
    document = {'format':'hoi4-guide-authoring','authoring_schema_version':1,'sources':[source],
                'guide':{'id':'word-'+hashlib.sha256(data).hexdigest()[:16], 'title':title,
                         'applies_to':{}, 'dependency_policy':'explicit',
                         'assumptions':['Draft generated from Word priority columns; review context, conditions, references and ID bindings. Other sections are retained in the source and are not converted to steps.'], 'steps':steps}}
    sources = [source]
    if context_entry:
        stream.seek(0)
        context = prepare_document(stream, context_entry)['sources'][0]
        if context['id'] == source['id']: raise ValueError('La fonte di contesto duplica la guida')
        sources.append(context)
        document['sources'] = sources
    return {'sources':sources, 'document':document, 'priority_tables':len(groups), 'embedded_images':media,
            'warnings':['La bozza usa solo le colonne prioritarie riconosciute; le altre sezioni restano nella fonte e richiedono revisione.',
                        'Solo alcune istruzioni esplicite sono strutturate; condizioni e indicazioni nei promemoria non sono monitorate automaticamente.']}
