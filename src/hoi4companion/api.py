"""Server HTTP locale; Pillow converte gli sprite. Non tocca i file di HOI4."""
from __future__ import annotations

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import mimetypes
import time
from pathlib import Path
from threading import RLock
from urllib.parse import urlparse, unquote, parse_qs
from .focus import evaluate, load_trees
from .installation import collect_files, active_mods, load_localisations
from .state import read_snapshot
from .dlc import discover_dlcs
from .eligibility import filter_trees, load_scripted_triggers
from .sprites import SpriteLibrary
from .technologies import load_technologies
from .live import LiveMonitor, NativeProvider
from .stream import LiveEventHub, SubscriberLimitError
from .technology_visuals import load_technology_visuals
from .guide import load_guide, list_guides, default_guide_id, enrich_guide, update_checks, manual_keys
from .guide_progress import RunStore
from .guide_runtime import evaluate as evaluate_guide
from .contracts import VERSION, normalise_state
from .guide_importer import Importer, catalog_entities
from .import_sources import acquire_source, ytdlp_path, MAX_BYTES
from .research_start import read_research_start
from .operation_import import operation_catalog

BASE = Path(__file__).resolve().parent
PROJECT = BASE.parent.parent
DEMO = PROJECT / 'examples' / 'demo_focus.txt'
WEB = BASE / 'web'
SETTINGS = PROJECT / '.local' / 'settings.json'


class Store:
    def __init__(self, overrides: dict | None = None):
        self.lock = RLock()
        self.settings: dict = {
            'game_dir': '', 'docs_dir': '', 'tree_id': '', 'snapshot_path': '',
            'country': 'AUS', 'language': 'italian', 'mods': 'auto',
        }
        if SETTINGS.exists():
            try:
                stored = json.loads(SETTINGS.read_text(encoding='utf-8'))
                self.settings.update({k: v for k, v in stored.items() if k in self.settings})
            except (OSError, ValueError):
                pass
        if overrides:
            self.settings.update({k: v for k, v in overrides.items() if v is not None and k in self.settings})
        self.trees: dict = {}
        self.warnings: list[str] = []
        self.demo = True
        self.import_error = ''
        self.dlcs = None
        self.excluded_trees = []
        self.sprites = None
        self.live_monitor = None
        self.live_provider = None
        self.live_provider_path = None
        self.live_cache = None
        self.live_cache_time = 0
        self.guide_checks = {}
        self.guide_revision = 0
        self.guide_states = {}
        self.guide_runs = RunStore()
        try:
            self.default_guide_id = default_guide_id()
        except (OSError, ValueError):
            # Optional content must never prevent the live framework starting.
            self.default_guide_id = None
        self.live_hub = LiveEventHub(lambda: self.live_state(force=True), interval=.25,
                                     on_idle=self.live_idle)
        self.refresh()

    def refresh(self):
        with self.lock:
            self.research_start_cache = {}
            config = self.settings
            game = Path(config['game_dir']).expanduser() if config['game_dir'] else None
            mods = []
            warnings = []
            self.dlcs = None
            self.excluded_trees = []
            self.sprites = None
            self.technologies = {'folders': {}, 'technologies': {}, 'warnings': [], 'excluded_folders': []}
            self.technology_visuals = {'templates': {}, 'folders': {}}
            self.operation_entities = []
            self.demo = not bool(game and (game / 'common' / 'national_focus').is_dir())
            if self.demo:
                files = [DEMO]
                labels = json.loads((PROJECT / 'examples' / 'demo_labels.json').read_text(encoding='utf-8'))
                reference_labels = labels
                warnings.append('Modalità dimostrativa: nessun file HOI4 reale caricato.')
            else:
                if config['mods'] == 'auto' and config['docs_dir']:
                    mods, warns = active_mods(Path(config['docs_dir']).expanduser())
                    warnings += warns
                files, warns = collect_files(game, mods, 'common/national_focus', '.txt')
                warnings += warns
                labels, reference_labels = load_localisations(game, mods, config['language'])
                self.operation_entities, issues = operation_catalog(game,mods,labels,reference_labels)
                warnings += issues
                if not files:
                    warnings.append('Nessun file di focus trovato; controlla il percorso di installazione.')
            trees, warns = load_trees(files, labels)
            warnings += warns
            if not self.demo:
                self.dlcs = discover_dlcs(game, Path(config['docs_dir']).expanduser() if config['docs_dir'] else None)
                warnings += self.dlcs.warnings
                trigger_files, warns = collect_files(game, mods, 'common/scripted_triggers', '.txt')
                warnings += warns
                scripts, warns = load_scripted_triggers(trigger_files)
                warnings += warns
                trees, self.excluded_trees = filter_trees(trees, self.dlcs, scripts)
                tech_files, warns = collect_files(game, mods, 'common/technologies', '.txt')
                warnings += warns
                tag_files, warns = collect_files(game, mods, 'common/technology_tags', '.txt')
                warnings += warns
                gui_files, warns = collect_files(game, mods, 'interface', '.gui')
                warnings += warns
                gui_files = [file for file in gui_files if file.name.lower().startswith('countrytechtree')]
                self.technologies = load_technologies(tech_files, tag_files, labels, self.dlcs, scripts,
                                                      config['country'].upper(), gui_files=gui_files)
                warnings += self.technologies['warnings']
                asset_roots = [game / Path(x['descriptor']).parent for x in self.dlcs.items
                               if x['status'] in ('enabled', 'integrated')]
                self.sprites = SpriteLibrary(game, [*asset_roots, *mods], PROJECT / '.local' / 'sprite-cache')
                warnings += self.sprites.warnings
                gfx_files, warns = collect_files(game, mods, 'interface', '.gfx')
                warnings += warns
                self.technology_visuals = load_technology_visuals(gui_files, gfx_files, self.sprites)
                warnings += self.technology_visuals['warnings']
            self.warnings = warnings
            self.labels = labels
            self.reference_labels = reference_labels
            self.game_root, self.mod_roots = game, mods
            self.scripts = scripts if not self.demo else {}
            self.trees = trees
            self.import_error = ''
            if self.settings['tree_id'] not in trees:
                self.settings['tree_id'] = next(iter(trees), '')

    def set_settings(self, update: dict):
        valid = {'game_dir', 'docs_dir', 'tree_id', 'snapshot_path', 'country', 'language', 'mods'}
        if not isinstance(update, dict) or any(k not in valid for k in update):
            raise ValueError('Impostazioni non riconosciute')
        if any(not isinstance(v, str) or len(v) > 2048 for v in update.values()):
            raise ValueError('Le impostazioni devono contenere solo stringhe')
        if 'country' in update and (len(update['country']) != 3 or not update['country'].isalpha()):
            raise ValueError('Codice paese non valido (esempio AUS)')
        if 'language' in update and update['language'] not in ('italian', 'english'):
            raise ValueError('Lingua non supportata')
        if 'mods' in update and update['mods'] not in ('auto', 'off'):
            raise ValueError('Impostazione mod non valida')
        with self.lock:
            self.settings.update(update)
            self.refresh()
            SETTINGS.parent.mkdir(exist_ok=True, parents=True)
            SETTINGS.write_text(json.dumps(self.settings, indent=2, ensure_ascii=False), encoding='utf-8')

    def overview(self):
        with self.lock:
            return {
                'settings': self.settings.copy(), 'demo': self.demo,
                'warnings': self.warnings[:70],
                'dlcs': self.dlcs.to_json() if self.dlcs else None,
                'excluded_trees': self.excluded_trees,
                'tree_filter': 'initial_dlc_compatibility' if not self.demo else 'demo',
                'trees': [{'id': x.id, 'count': len(x.focuses), 'source': x.source, 'countries': x.eligible_countries}
                          for x in self.trees.values()],
            }

    def tree(self, tree_id: str | None, include_snapshot=True):
        with self.lock:
            target = tree_id or self.settings['tree_id']
            tree = self.trees.get(target)
            if not tree:
                return {'error': 'Albero non trovato'}
            snapshot = None
            error = ''
            p = self.settings['snapshot_path']
            if p and include_snapshot:
                try:
                    snapshot = read_snapshot(Path(p).expanduser(), self.settings['country'].upper())
                except (OSError, ValueError, RecursionError) as exc:
                    error = str(exc)
            statuses = evaluate(tree, set(snapshot.completed), snapshot.current) if snapshot else {}
            result = tree.to_json()
            if self.sprites:
                texts = []
                for focus in result['focuses']:
                    focus['sprite'] = self.sprites.resolve(focus['icon']) if focus['icon'] else None
                    focus['variant_sprites'] = [sprite for name in focus['icon_variants']
                                                if (sprite := self.sprites.resolve(name))]
                    texts += [focus['name'], focus['description'], *(x['text'] for x in focus['tooltip_sections'])]
                result['inline_sprites'] = self.sprites.inline(texts)
                result['theme'] = self.sprites.theme()
                result['asset_coverage'] = {'resolved': sum(bool(f['sprite']) for f in result['focuses']),
                                            'conditional': sum(bool(f['icon_variants']) for f in result['focuses']),
                                            'total': len(result['focuses'])}
            return {**result, 'status': statuses,
                    'snapshot': snapshot.to_json() if snapshot else None,
                    'snapshot_error': error, 'demo': self.demo,
                    'has_snapshot': bool(snapshot)}

    def sprite(self, token: str):
        with self.lock:
            return self.sprites.png(token) if self.sprites else None

    def live_idle(self):
        with self.lock:
            if getattr(self, 'live_provider', None) is not None:
                self.live_provider.close()
            self.live_cache = None

    def close(self):
        # Never hold Store.lock while joining the producer: its idle callback
        # releases the read-only handle using that same lock.
        self.live_hub.close()
        self.live_idle()

    def live_state(self, force=False):
        with self.lock:
            path = self.settings['game_dir']
            if self.live_monitor is None or self.live_provider_path != path:
                if getattr(self, 'live_provider', None) is not None:
                    self.live_provider.close()
                self.live_provider = NativeProvider(Path(path) / 'hoi4.exe')
                self.live_monitor = LiveMonitor(self.live_provider.collect)
                self.live_provider_path = path
                self.live_cache = None
            now = time.monotonic()
            if force or self.live_cache is None or now - self.live_cache_time >= .25:
                result = self.live_monitor.poll()
                if result['usable']:
                    ids = set(result['sample']['focus_ids'])
                    matches = [key for key, tree in self.trees.items() if set(tree.focuses) == ids]
                    native_id = result['sample'].get('native_focus_tree_id')
                    matched = native_id if native_id in self.trees and set(self.trees[native_id].focuses) <= ids else matches[0] if len(matches) == 1 else None
                    result['sample']['focus_tree_id'] = matched
                    result['sample']['tree_identity_basis'] = 'native_id_and_membership' if matched == native_id and matched else 'exact_native_focus_membership' if matched else 'unresolved'
                    sample = result['sample'] = {**result['sample'],
                                                'research_slots': [dict(slot) for slot in result['sample'].get('research_slots', [])]}
                    labels = getattr(self, 'labels', {})
                    sample['focus_name'] = labels.get(sample.get('current_focus'))
                    sample['continuous_focus_name'] = labels.get(sample.get('continuous_focus'))
                    for slot in sample.get('research_slots', []):
                        slot['name'] = labels.get(slot['technology_id'], slot['technology_id'])
                    if sample.get('country_overview'):
                        from .country import localise_country
                        sample['country_overview'] = localise_country(sample['country_overview'], labels)
                provider = getattr(self, 'live_provider', None)
                if provider is not None:
                    result['metrics'] = dict(provider.metrics)
                result['transport'] = {'kind': 'change_events', 'memory_probe_seconds': .25,
                                       'integrity_audit_seconds': 5}
                self.live_cache, self.live_cache_time = result, now
            self._bind_progress()
            default_id = getattr(self, 'default_guide_id', None)
            checks = getattr(self, 'guide_checks', {}).copy()
            states = {key: dict(value) for key, value in getattr(self, 'guide_states', {}).items()}
            if default_id:
                states[default_id] = checks
            return {**self.live_cache, 'guide_id': default_id, 'guide_checks': checks,
                    'guide_states': states,
                    'guide_started': {key: self._progress_store().get(key)['started']
                                      for key in self._progress_store().metadata()},
                    'guide_baselines': {key: self._progress_store().get(key).get('baselines', {})
                                        for key in self._progress_store().metadata()},
                    'guide_runs': self._progress_store().metadata(),
                    'guide_revision': getattr(self,'guide_revision',0)}

    def _progress_store(self):
        if not hasattr(self, 'guide_runs'):
            self.guide_runs = RunStore()
        return self.guide_runs

    def _sync_progress(self):
        runs = self._progress_store()
        if not hasattr(self, 'guide_states'):
            self.guide_states = {}
        for guide_id in runs.metadata():
            checks = runs.get(guide_id)['checks']
            self.guide_states[guide_id] = checks
            if guide_id == getattr(self, 'default_guide_id', None):
                self.guide_checks = checks.copy()

    def _bind_progress(self):
        # Consumer metadata only: never acquire or infer a native identity here.
        cached = getattr(self, 'live_cache', None)
        session = (cached.get('sample') or {}).get('session_id') if cached and cached.get('usable') else None
        if self._progress_store().bind(session):
            self._sync_progress()
            self.guide_revision = getattr(self, 'guide_revision', 0) + 1
        if session:
            for guide_id in self._progress_store().metadata():
                if self._progress_store().observe(load_guide(guide_id), cached):
                    self.guide_revision = getattr(self, 'guide_revision', 0) + 1

    def _ensure_progress(self, guide_id):
        default_id = getattr(self, 'default_guide_id', None) or default_guide_id()
        self.default_guide_id = default_id
        checks = getattr(self, 'guide_checks', {}) if guide_id == default_id else getattr(self, 'guide_states', {}).get(guide_id, {})
        self._progress_store().ensure(guide_id, checks)
        self._bind_progress()

    def walkthrough_progress(self, update=None, guide_id=None, run_id=None):
        with self.lock:
            guide = load_guide(guide_id or getattr(self, 'default_guide_id', None))
            self._ensure_progress(guide['id'])
            runs = self._progress_store()
            if update is not None:
                if not isinstance(update, dict):
                    raise ValueError('Avanzamento della partita non valido')
                if update == {}:
                    runs.create(guide['id'], runs.get(guide['id'])['session_id'])
                elif set(update) == {'run_id'}:
                    runs.select(guide['id'], update['run_id'])
                elif ('run_id' in update and set(update) <= {'run_id', 'checks', 'started'}
                      and ('checks' in update or 'started' in update)):
                    current = runs.get(guide['id'])
                    if update['run_id'] != current['run_id']:
                        raise ValueError('Checklist di una partita non attiva')
                    checks = update.get('checks', current['checks'])
                    if not isinstance(checks, dict) or set(checks) - manual_keys(guide):
                        raise ValueError('Conferme non dichiarate dalla guida')
                    started = update.get('started', current['started'])
                    operations = {step['key'] for rows in guide['tracks'].values() for step in rows
                                  if step['action']['kind'] in ('manual', 'special_project') and step.get('tracking') != 'reminder'}
                    if not isinstance(started, dict) or set(started) - operations:
                        raise ValueError('Avvii non dichiarati dalla guida')
                    runs.update(guide['id'], checks, started=started)
                else:
                    raise ValueError('Operazione sulla partita non riconosciuta')
                self._sync_progress()
                self.guide_revision = getattr(self, 'guide_revision', 0) + 1
            return {**runs.get(guide['id'], run_id), 'revision': getattr(self, 'guide_revision', 0)}

    def walkthrough_runtime(self, guide_id=None, run_id=None):
        with self.lock:
            state = self.live_state()
            guide = self.walkthrough(guide_id)
            progress = self.walkthrough_progress(guide_id=guide['id'], run_id=run_id)
            return evaluate_guide(guide, state, progress)

    def walkthrough_checks(self, update=None, guide_id=None):
        with self.lock:
            default_id = getattr(self, 'default_guide_id', None) or default_guide_id()
            guide = load_guide(guide_id or default_id)
            self._ensure_progress(guide['id'])
            current = self._progress_store().get(guide['id'])['checks']
            if update is not None:
                current = update_checks(guide,current,update)
                self._progress_store().update(guide['id'], current,
                                              started={} if update == {'reset': True} else None)
                self._sync_progress()
                self.guide_revision += 1
            return {'checks':current.copy(),'revision':self.guide_revision}

    def walkthrough(self, guide_id=None):
        with self.lock:
            result = enrich_guide(load_guide(guide_id or getattr(self, 'default_guide_id', None)), self.trees, self.technologies['technologies'],
                                  self.labels, self.technologies['folders'])
            result['catalog_country'] = self.settings['country'].upper()
            result['demo'] = self.demo
            return result

    def importer(self):
        with self.lock:
            if not hasattr(self, '_importer'):
                self._importer = Importer(self.import_catalog, research_context=self.research_start)
            return self._importer

    def research_start(self, guide):
        with self.lock:
            if self.demo or not getattr(self,'game_root',None): return None
            scope = guide.get('applies_to',{})
            if not isinstance(scope,dict): return None
            country = scope.get('country',guide.get('country'))
            date = guide.get('start_date')
            if not isinstance(country,str) or date is not None and not isinstance(date,str): return None
            key = (country,date)
            if key not in self.research_start_cache:
                self.research_start_cache[key] = read_research_start(self.game_root,self.mod_roots,country,self.dlcs,self.scripts,date)
            return self.research_start_cache[key]

    def import_catalog(self):
        with self.lock:
            return catalog_entities(self.trees, self.technologies['technologies'], self.labels,
                                    getattr(self, 'reference_labels', {})) + getattr(self,'operation_entities',[])

    def technology_catalog(self):
        with self.lock:
            folders = []
            for folder in self.technologies['folders'].values():
                item = {k: v for k, v in folder.items() if k != 'technologies'} | {'count': len(folder['technologies'])}
                if folder.get('tab'):
                    item['tab'] = {**folder['tab'], 'sprite': self.sprites.resolve(folder['tab']['sprite_name']) if self.sprites else None}
                folders.append(item)
            return {'folders': folders,
                    'excluded_folders': self.technologies['excluded_folders'],
                    'country': self.settings['country'].upper(), 'warnings': self.technologies['warnings'],
                    'demo': self.demo}

    def technology_tree(self, folder_id):
        with self.lock:
            folder = self.technologies['folders'].get(folder_id)
            if not folder:
                return {'error': 'Categoria di ricerca non trovata'}
            catalog = self.technologies['technologies']
            ids = set(folder['technologies'])
            # Include folderless children in details, with bounded traversal for mod cycles.
            related = set(ids)
            pending = list(ids)
            while pending:
                item = catalog.get(pending.pop())
                for child in item['sub_technologies'] if item else []:
                    if child in catalog and child not in related:
                        related.add(child)
                        pending.append(child)
            result = []
            country = self.settings['country'].upper()
            for key in sorted(related):
                tech = dict(catalog[key])
                tech['in_folder'] = key in ids
                tech['sprite'] = None
                membership = next((f for f in tech['folders'] if f['id'] == folder_id), {})
                template = membership.get('layout', {}).get('template')
                tech['visual'] = getattr(self, 'technology_visuals', {}).get('templates', {}).get(template)
                if self.sprites:
                    compact = membership.get('layout', {}).get('variant') == 'small'
                    names = (f'GFX_{country}_{key}', f'GFX_{key}', f'GFX_{country}_{key}_medium', f'GFX_{key}_medium') if compact else (f'GFX_{country}_{key}_medium', f'GFX_{key}_medium', f'GFX_{country}_{key}', f'GFX_{key}')
                    for name in names:
                        tech['sprite'] = self.sprites.resolve(name)
                        if tech['sprite']:
                            break
                result.append(tech)
            reference_ids = {ref for t in result for ref in
                             [*(p['target'] for p in t['paths']), *(d['id'] for d in t['dependencies']), *t['exclusions']]}
            references = {}
            for key in reference_ids - related:
                target = catalog.get(key)
                if target:
                    destination = next((f['id'] for f in target['folders'] if f['id'] in self.technologies['folders']), None)
                    references[key] = {'name': target['name'], 'folder': destination}
            return {'id': folder['id'], 'name': folder['name'], 'technologies': result, 'references': references,
                    'layout': folder.get('layout'), 'sections': folder.get('sections', []),
                    'visuals': getattr(self, 'technology_visuals', {}).get('folders', {}).get(folder_id, {}),
                    'country': country, 'inline_sprites': self.sprites.inline([s for t in result for s in (t['name'], t['description'])]) if self.sprites else {},
                    'asset_coverage': {'resolved': sum(bool(t['sprite']) for t in result if t['in_folder']), 'total': len(ids)}}


def make_handler(store: Store):
    class Handler(BaseHTTPRequestHandler):
        def _send(self, status: int, data: bytes, mime: str):
            self.send_response(status)
            self.send_header('Content-Type', mime)
            self.send_header('Content-Length', str(len(data)))
            self.send_header('X-Content-Type-Options', 'nosniff')
            self.send_header('Cache-Control', 'no-store')
            self.send_header('Content-Security-Policy', "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'")
            self.end_headers()
            self.wfile.write(data)

        def json(self, data, status=200):
            self._send(status, json.dumps(data, ensure_ascii=False).encode('utf-8'), 'application/json; charset=utf-8')

        def do_GET(self):
            url = urlparse(self.path)
            if url.path == '/api/import/status':
                self.json({**store.importer().status(), 'youtube_available': bool(ytdlp_path())})
                return
            if url.path == '/api/import/prompt':
                self.json({'prompt': store.importer().prompt()})
                return
            if url.path == '/api/import/catalog':
                self.json({'entities': store.importer().catalog(), 'demo': store.demo})
                return
            if url.path == '/api/state/events':
                origin, host = self.headers.get('Origin', ''), self.headers.get('Host', '')
                if (origin and urlparse(origin).netloc != host) or self.headers.get('Sec-Fetch-Site') == 'cross-site':
                    self.json({'error': 'Origine non autorizzata'}, 403)
                    return
                try:
                    subscription = store.live_hub.subscribe()
                except SubscriberLimitError:
                    self.json({'error': 'Troppe viste live aperte'}, 503)
                    return
                with subscription as client:
                    try:
                        self.connection.settimeout(10)
                        self.send_response(200)
                        self.send_header('Content-Type', 'text/event-stream; charset=utf-8')
                        self.send_header('Cache-Control', 'no-cache, no-transform')
                        self.send_header('X-Content-Type-Options', 'nosniff')
                        self.send_header('Connection', 'close')
                        self.end_headers()
                        self.close_connection = True
                        self.wfile.write(b'retry: 1000\n\n')
                        self.wfile.flush()
                        while not store.live_hub.closed:
                            event = client.wait(5)
                            if event is not None:
                                payload = json.dumps(event.value, ensure_ascii=False, separators=(',', ':'))
                                frame = f'id: {event.revision}\nevent: state\ndata: {payload}\n\n'
                            else:
                                payload = json.dumps(store.live_hub.heartbeat(), separators=(',', ':'))
                                frame = f'event: heartbeat\ndata: {payload}\n\n'
                            self.wfile.write(frame.encode('utf-8'))
                            self.wfile.flush()
                    except (BrokenPipeError, ConnectionError, TimeoutError, OSError):
                        pass
                return
            if url.path.startswith('/api/sprite/'):
                import re
                match = re.fullmatch(r'/api/sprite/([a-f0-9]{64})\.png', url.path)
                data = store.sprite(match.group(1)) if match else None
                if data:
                    self._send(200, data, 'image/png')
                else:
                    self.json({'error': 'Sprite non disponibile'}, 404)
                return
            if url.path == '/api/config':
                self.json(store.overview())
                return
            if url.path == '/api/state':
                self.json(store.live_state())
                return
            if url.path == '/api/framework/state':
                self.json(normalise_state(store.live_state()))
                return
            if url.path == '/api/guides':
                try:
                    self.json({'schema_version': VERSION, 'guides': list_guides()})
                except (OSError, ValueError) as exc:
                    self.json({'error': str(exc)}, 400)
                return
            if url.path == '/api/guide':
                try:
                    self.json(store.walkthrough(parse_qs(url.query).get('id', [None])[0]))
                except (OSError, ValueError) as exc:
                    self.json({'error': str(exc)}, 400)
                return
            if url.path == '/api/guide/checks':
                try:
                    self.json(store.walkthrough_checks(guide_id=parse_qs(url.query).get('id', [None])[0]))
                except (OSError, ValueError) as exc:
                    self.json({'error': str(exc)}, 400)
                return
            if url.path in ('/api/guide/progress', '/api/guide/runtime'):
                try:
                    query = parse_qs(url.query)
                    method = store.walkthrough_runtime if url.path.endswith('/runtime') else store.walkthrough_progress
                    self.json(method(guide_id=query.get('id', [None])[0], run_id=query.get('run', [None])[0]))
                except (OSError, ValueError) as exc:
                    self.json({'error': str(exc)}, 400)
                return
            if url.path == '/api/tree':
                query = parse_qs(url.query)
                self.json(store.tree(query.get('id', [None])[0], include_snapshot=query.get('source') != ['live']))
                return
            if url.path == '/api/technologies':
                self.json(store.technology_catalog())
                return
            if url.path == '/api/technology-tree':
                self.json(store.technology_tree(parse_qs(url.query).get('id', [''])[0]))
                return
            static = {'/': 'index.html', '/index.html': 'index.html',
                      '/importer': 'importer.html', '/importer.js': 'importer.js',
                      '/importer-model.js': 'importer-model.js', '/importer.css': 'importer.css',
                      '/contracts.js': 'contracts.js',
                      '/country': 'country.html', '/country.js': 'country.js',
                      '/country.css': 'country.css', '/country-model.js': 'country-model.js',
                      '/walkthrough': 'guide.html', '/hud': 'guide.html', '/guide-model.js': 'guide-model.js',
                      '/guide.js': 'guide.js', '/guide.css': 'guide.css',
                      '/guide-runtime.js': 'guide-runtime.js',
                      '/production-progress.js': 'production-progress.js',
                      '/guide-presentation.js': 'guide-presentation.js', '/guide-session.js': 'guide-session.js',
                      '/guide-assist.js': 'guide-assist.js', '/guide-assist.css': 'guide-assist.css',
                      '/live-state.js': 'live-state.js', '/live-state.css': 'live-state.css', '/live-model.js': 'live-model.js',
                      '/technologies': 'technologies.html', '/technologies.js': 'technologies.js', '/technologies.css': 'technologies.css',
                      '/explorer.js': 'explorer.js', '/style.css': 'style.css', '/game.css': 'game.css'}
            file = static.get(url.path)
            if file:
                data = (WEB / file).read_bytes()
                mime = mimetypes.guess_type(file)[0] or 'application/octet-stream'
                if mime.startswith('text/') or 'javascript' in mime:
                    mime += '; charset=utf-8'
                self._send(200, data, mime)
            else:
                self.json({'error': 'Not found'}, 404)

        def do_POST(self):
            url = urlparse(self.path)
            import_paths = {'/api/import/source', '/api/import/packet', '/api/import/extract', '/api/import/preview', '/api/import/install', '/api/import/document'}
            if url.path not in ('/api/config', '/api/refresh', '/api/guide/checks', '/api/guide/progress') and url.path not in import_paths:
                self.json({'error': 'Not found'}, 404)
                return
            # Blocca richieste cross-origin da siti terzi verso il servizio locale.
            origin = self.headers.get('Origin', '')
            host = self.headers.get('Host', '')
            if (origin and urlparse(origin).netloc != host) or self.headers.get('Sec-Fetch-Site') == 'cross-site':
                self.json({'error': 'Origine non autorizzata'}, 403)
                return
            if self.headers.get('Content-Type', '').split(';')[0] != 'application/json':
                self.json({'error': 'Richiesta JSON necessaria'}, 415)
                return
            try:
                length = int(self.headers.get('Content-Length', '0'))
            except ValueError:
                length = 0
            if length < 1 or length > (MAX_BYTES if url.path in import_paths else 16_384):
                self.json({'error': 'Dimensione non valida'}, 413)
                return
            try:
                update = json.loads(self.rfile.read(length))
                if url.path in import_paths:
                    if not isinstance(update, dict):
                        raise ValueError('Richiesta importazione non valida')
                    operation = url.path.rsplit('/', 1)[-1]
                    allowed = {'source': {'url', 'text'}, 'packet': {'sources', 'tree_id'},
                               'extract': {'sources', 'tree_id'}, 'preview': {'guide', 'sources'},
                               'install': {'guide', 'sources', 'reviewed'}, 'document': {'document'}}[operation]
                    if set(update) - allowed:
                        raise ValueError('Campi importazione non riconosciuti')
                    importer = store.importer()
                    if operation == 'source':
                        result = acquire_source(update.get('url', ''), update.get('text', ''))
                    elif operation == 'document':
                        result = importer.document(update.get('document'))
                    elif operation == 'packet':
                        result = importer.packet(update.get('sources'), update.get('tree_id'))
                    elif operation == 'extract':
                        result = importer.extract(update.get('sources'), update.get('tree_id'))
                    elif operation == 'preview':
                        result = importer.preview(update.get('guide'), update.get('sources'))
                    else:
                        result = importer.install(update.get('guide'), update.get('sources'), reviewed=update.get('reviewed'))
                    self.json(result)
                    return
                if url.path == '/api/guide/checks':
                    self.json(store.walkthrough_checks(update, parse_qs(url.query).get('id', [None])[0]))
                    return
                if url.path == '/api/guide/progress':
                    self.json(store.walkthrough_progress(update, parse_qs(url.query).get('id', [None])[0]))
                    return
                if url.path == '/api/refresh':
                    if update != {}:
                        raise ValueError('Il ricaricamento richiede un oggetto vuoto')
                    store.refresh()
                else:
                    store.set_settings(update)
            except (ValueError, UnicodeError, OSError, RecursionError) as exc:
                self.json({'error': str(exc)}, 400)
                return
            self.json(store.overview())

        def log_message(self, fmt, *args):
            # Evita rumore per polling periodico, mantiene errori utili.
            if not self.path.startswith(('/api/tree', '/api/state', '/api/sprite/')):
                super().log_message(fmt, *args)

    return Handler


def serve(store: Store, host='127.0.0.1', port=8766, open_browser=True):
    import webbrowser
    with ThreadingHTTPServer((host, port), make_handler(store)) as server:
        url = f'http://{host}:{port}'
        print(f'HOI4 Companion disponibile su {url} (Ctrl+C per uscire)')
        if open_browser:
            webbrowser.open(url)
        try:
            server.serve_forever(poll_interval=0.3)
        except KeyboardInterrupt:
            print('\nServer arrestato.')
        finally:
            store.close()
