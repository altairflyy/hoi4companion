"""Bounded, passive source acquisition. No browser, login, media or game access."""
from __future__ import annotations

import hashlib
from html.parser import HTMLParser
import http.client
import ipaddress
import json
import os
from pathlib import Path
import re
import shutil
import socket
import ssl
import subprocess
import tempfile
import sys
from .paths import module_command,bundled_node
from urllib.parse import urlsplit, urljoin, parse_qs

MAX_TEXT = 180_000
MAX_BYTES = 2_000_000


def public_url(url):
    if not isinstance(url, str) or len(url) > 4096 or any(c.isspace() or ord(c) < 32 for c in url):
        raise ValueError('URL non valido')
    p = urlsplit(url)
    if p.scheme not in ('http', 'https') or not p.hostname or p.username or p.password or p.port not in (None, 80, 443):
        raise ValueError('Usa un URL HTTP/HTTPS pubblico senza credenziali e porte personalizzate')
    addresses = socket.getaddrinfo(p.hostname, p.port or (443 if p.scheme == 'https' else 80), type=socket.SOCK_STREAM)
    if not addresses or any(not ipaddress.ip_address(item[4][0]).is_global for item in addresses):
        raise ValueError('Indirizzi locali, privati o riservati non consentiti')
    return p, addresses[0][4][0]


def fetch_public(url, redirects=0):
    """Pin the validated IP while retaining hostname for TLS and Host; no proxy."""
    if redirects > 4:
        raise ValueError('Troppi reindirizzamenti')
    p, address = public_url(url)
    port = p.port or (443 if p.scheme == 'https' else 80)
    connection = http.client.HTTPConnection(p.hostname, port, timeout=18)
    try:
        connection.sock = socket.create_connection((address, port), timeout=18)
        if p.scheme == 'https':
            connection.sock = ssl.create_default_context().wrap_socket(connection.sock, server_hostname=p.hostname)
        connection.request('GET', (p.path or '/') + ('?' + p.query if p.query else ''),
                           headers={'User-Agent': 'HOI4Companion-GuideImporter/1', 'Accept-Encoding': 'identity'})
        response = connection.getresponse()
        if response.status in (301, 302, 303, 307, 308):
            location = response.getheader('Location')
            if not location:
                raise ValueError('Reindirizzamento privo di destinazione')
            return fetch_public(urljoin(url, location), redirects + 1)
        if response.status != 200:
            raise ValueError(f'Fonte non accessibile (HTTP {response.status}); puoi incollare il testo')
        data = response.read(MAX_BYTES + 1)
        if len(data) > MAX_BYTES or response.getheader('Content-Encoding', 'identity') != 'identity':
            raise ValueError('Fonte troppo grande o codifica non supportata')
        return data, response.getheader('Content-Type', ''), url
    finally:
        connection.close()


class TextHTML(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts, self.hidden = [], 0

    def handle_starttag(self, tag, attrs):
        if tag in ('script', 'style', 'noscript', 'svg', 'nav', 'footer'):
            self.hidden += 1
        if not self.hidden and tag in ('p', 'br', 'div', 'h1', 'h2', 'h3', 'li', 'tr', 'article'):
            self.parts.append('\n')

    def handle_endtag(self, tag):
        if tag in ('script', 'style', 'noscript', 'svg', 'nav', 'footer') and self.hidden:
            self.hidden -= 1
        if not self.hidden:
            self.parts.append(' ')

    def handle_data(self, data):
        if not self.hidden:
            self.parts.append(data)


def parse_html(html):
    parser = TextHTML(); parser.feed(html)
    return '\n'.join(' '.join(line.split()) for line in ''.join(parser.parts).splitlines() if line.strip())


def parse_captions(value):
    rows = []
    for event in value.get('events', []):
        text = ''.join(seg.get('utf8', '') for seg in event.get('segs', []))
        if text.strip() and type(event.get('tStartMs')) in (int, float) and event['tStartMs'] >= 0:
            rows.append({'start': event['tStartMs'] / 1000, 'text': text.strip()})
    return rows


def youtube_url(url):
    p = urlsplit(url)
    if p.scheme != 'https' or p.hostname not in ('youtube.com', 'www.youtube.com', 'm.youtube.com', 'youtu.be') or p.username or p.password or p.port:
        raise ValueError('URL YouTube non valido')
    key = p.path.strip('/') if p.hostname == 'youtu.be' else parse_qs(p.query).get('v', [''])[0]
    if p.path.startswith(('/shorts/', '/live/')):
        key = p.path.split('/')[2]
    if not re.fullmatch(r'[A-Za-z0-9_-]{11}', key):
        raise ValueError('Indica un singolo video YouTube')
    return 'https://www.youtube.com/watch?v=' + key


def ytdlp_path():
    configured = os.environ.get('HOI4_YTDLP')
    executable = configured or shutil.which('yt-dlp')
    if not executable:
        downloaded = Path.home() / 'Downloads' / 'yt-dlp.exe'
        executable = str(downloaded) if downloaded.is_file() else None
    return executable


def youtube_source(url):
    canonical = youtube_url(url)
    executable = ytdlp_path()
    launcher=module_command('yt_dlp') if getattr(sys,'frozen',False) and not os.environ.get('HOI4_YTDLP') else [executable] if executable else []
    if not launcher:
        raise ValueError('yt-dlp non trovato: configura HOI4_YTDLP oppure incolla descrizione e trascrizione')
    # No user arguments, configs, plugins, cookies, playlist or media download.
    with tempfile.TemporaryFile() as output:
        try:
            result = subprocess.run([*launcher, '--ignore-config', '--no-plugin-dirs', '--no-playlist',
                *(['--js-runtimes','node:'+str(bundled_node())] if bundled_node() else []),
                '--skip-download', '--dump-single-json', '--socket-timeout', '12', '--retries', '1', '--', canonical],
                stdout=output, stderr=subprocess.DEVNULL, timeout=55,
                creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        except (OSError, subprocess.TimeoutExpired) as exc:
            raise ValueError('Acquisizione YouTube non riuscita; incolla descrizione e trascrizione') from exc
        if result.returncode:
            raise ValueError('YouTube non accessibile con yt-dlp; nessun tentativo di login. Puoi incollare il testo')
        if output.tell() > 12_000_000:
            raise ValueError('Metadati YouTube troppo grandi')
        output.seek(0); metadata = json.load(output)
    warnings, segments = ['Il video non è analizzato visivamente; i contenuti mostrati solo nei fotogrammi possono mancare.'], []
    for collection in (metadata.get('subtitles', {}), metadata.get('automatic_captions', {})):
        languages = sorted(collection, key=lambda key: (not key.startswith('it'), not key.startswith('en'), key))
        for language in languages:
            caption = next((item for item in collection[language] if item.get('ext') == 'json3'), None)
            if caption:
                try:
                    data, _, _ = fetch_public(caption['url'])
                    segments = parse_captions(json.loads(data))
                    if segments:
                        warnings.append(f'Sottotitoli: {language}; possono contenere errori o essere automatici.')
                        break
                except (OSError, ValueError, http.client.HTTPException):
                    warnings.append('Sottotitoli non scaricabili; acquisita la descrizione disponibile.')
                    break
        if segments:
            break
    if not segments:
        warnings.append('Trascrizione assente: estrazione limitata alla descrizione.')
    text = metadata.get('description') or ''
    if segments:
        text += '\n\nTRASCRIZIONE\n' + '\n'.join(f"[{row['start']:g}s] {row['text']}" for row in segments)
    return canonical, metadata.get('title', 'Video YouTube'), text, segments, warnings


def acquire_source(url='', text=''):
    if not isinstance(url, str) or not isinstance(text, str):
        raise ValueError('Fonte non valida')
    warnings, segments, title, kind = [], [], 'Testo fornito', 'manual'
    if text.strip():
        if url:
            p = urlsplit(url)
            if p.scheme not in ('http', 'https') or not p.hostname or p.username or p.password:
                raise ValueError('URL fonte non valido')
            kind = 'youtube' if p.hostname in ('youtube.com', 'www.youtube.com', 'youtu.be') else 'web'
        warnings.append('Testo fornito manualmente, non verificato contro la pagina remota.')
    elif urlsplit(url).hostname in ('youtube.com', 'www.youtube.com', 'm.youtube.com', 'youtu.be'):
        url, title, text, segments, warnings = youtube_source(url); kind = 'youtube'
    elif url:
        try:
            data, mime, url = fetch_public(url)
        except (OSError, http.client.HTTPException) as exc:
            raise ValueError('Fonte non raggiungibile; puoi incollare il testo') from exc
        encoding = re.search(r'charset=([\w-]+)', mime)
        try:
            decoded = data.decode(encoding[1] if encoding else 'utf-8', errors='replace')
        except LookupError as exc:
            raise ValueError('Codifica della fonte non supportata') from exc
        if 'html' in mime:
            text = parse_html(decoded)
        elif 'text/plain' in mime:
            text = decoded
        else:
            raise ValueError('Usa una pagina HTML o testo; PDF e contenuti con login vanno convertiti in testo')
        title, kind = urlsplit(url).hostname, 'web'
        warnings.append('Estratto il testo HTML statico; script, immagini e contenuti con login non acquisiti.')
    if not text.strip() or len(text) > MAX_TEXT:
        raise ValueError(f'Testo fonte vuoto o maggiore di {MAX_TEXT} caratteri: suddividi la fonte')
    digest = hashlib.sha256(text.encode()).hexdigest()
    value = {'id': 'src-' + digest[:16], 'type': kind, 'title': title, 'text': text,
             'content_sha256': digest, 'segments': segments, 'warnings': warnings}
    if url:
        value['url'] = url
    return value
