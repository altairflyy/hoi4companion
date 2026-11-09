"""Server HTTP locale; Pillow converte gli sprite. Non tocca i file di HOI4."""
from __future__ import annotations

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import mimetypes
import re
import time
from pathlib import Path
from threading import RLock
from uuid import uuid4
from urllib.parse import urlparse, unquote, parse_qs
from .focus import evaluate, load_trees, load_continuous_focuses
from .installation import collect_files, inspect_mods, load_localisations, detect_game, default_documents
from .state import read_snapshot
from .dlc import discover_dlcs
from .eligibility import filter_trees, load_scripted_triggers, excluded_focus_branches
from .sprites import SpriteLibrary
from .technologies import load_technologies
from .live import LiveMonitor, NativeProvider
from .stream import LiveEventHub, SubscriberLimitError
from .technology_visuals import load_technology_visuals
from .focus_visuals import load_focus_visuals
from .overlay_diagnostics import HudStatus
from .guide import load_guide, list_guides, default_guide_id, enrich_guide, update_checks, manual_keys
from .guide_progress import RunStore
from .guide_runtime import evaluate as evaluate_guide
from .contracts import VERSION, normalise_state, GuideProjection
from .guide_importer import Importer, catalog_entities
from .import_sources import acquire_source, ytdlp_path, MAX_BYTES
from .research_start import read_research_start
from .operation_import import operation_catalog
from .jomini_parser import backend_info
from .guide_media import VideoJobs
from .guide_frames import GuideFrames, frame_reference, media_references
from .paths import data_directory

BASE = Path(__file__).resolve().parent
PROJECT = BASE.parent.parent
DEMO = BASE / 'examples' / 'demo_focus.txt'
WEB = BASE / 'web'
SETTINGS = data_directory() / 'settings.json'


class Store:
    def __init__(self, overrides: dict | None = None):
        # Deployment errors must abort before file loaders can treat them as
        # malformed scripts, or progress/campaign state can be initialized.
        self.parser_info = backend_info()
        self.lock = RLock()
        self._analysis_lock = RLock()
        self._analysis_report = None
        self._analysis_generation = 0
        self.settings: dict = {
            'game_dir': '', 'docs_dir': '', 'tree_id': '', 'snapshot_path': '',
            'country': 'AUS', 'language': 'english', 'ui_language': 'english', 'mods': 'auto',
        }
        if SETTINGS.exists():
            try:
                stored = json.loads(SETTINGS.read_text(encoding='utf-8'))
                self.settings.update({k: v for k, v in stored.items() if k in self.settings})
            except (OSError, ValueError):
                pass
        if overrides:
            self.settings.update({k: v for k, v in overrides.items() if v is not None and k in self.settings})
        # Explicit/persisted paths win over discovery, including a custom HOI4
        # profile. Discovery only fills empty settings for this installation.
        if not self.settings['game_dir']:
            detected = detect_game()
            if detected:
                self.settings['game_dir'] = str(detected)
        if not self.settings['docs_dir']:
            self.settings['docs_dir'] = str(default_documents())
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
        self.video_jobs = VideoJobs(SETTINGS.parent / 'guide-videos')
        self.guide_frames = GuideFrames(SETTINGS.parent / 'guide-frames')
        from .reference_designs import ReferenceLibrary
        self.reference_library=ReferenceLibrary(SETTINGS.parent/'reference-library')
        self.guide_revision = 0
        self.guide_states = {}
        self.guide_server_id=uuid4().hex
        self.progress_error=''
        try:self.guide_runs=RunStore(path=SETTINGS.with_name('guide-progress.sqlite3'))
        except ValueError as exc:
            self.guide_runs=RunStore();self.progress_error=str(exc)
        try:
            self.default_guide_id = default_guide_id()
        except (OSError, ValueError):
            # Optional content must never prevent the live framework starting.
            self.default_guide_id = None
        self.active_guide_id=self.default_guide_id
        self.active_guide_revision=0
        self.hud_status=HudStatus(SETTINGS.with_name('overlay-status.json'))
        try:
            selection=json.loads(SETTINGS.with_name('active-guide.json').read_text(encoding='utf-8'))
            self.active_guide_id=load_guide(selection.get('guide_id'))['id']
        except (OSError, ValueError, AttributeError):pass
        self._sync_progress()
        self.live_hub = LiveEventHub(lambda: self.live_state(force=True), interval=.25,
                                     on_idle=self.live_idle)
        self.refresh()

    def refresh(self):
        from .catalog_generation import CatalogGeneration
        with self.lock:
            owner=CatalogGeneration(self.settings,SETTINGS.parent/'sprite-cache',DEMO)
            attributes,settings=owner.attributes(SETTINGS.parent/'sprite-cache')
            # Publish only after the complete replacement has been decoded.
            self.__dict__.update(attributes)
            self._catalog_generation=owner
            self.settings.update(settings)
            self._resolution_generation=None
            self._relative_legacy_index=None
            self._relative_legacy_cache={}
            self.research_start_cache={}
            self._compatibility_engine=None
            self._word_compatibility_cache={}
            self._analysis_generation=getattr(self,'_analysis_generation',0)+1
            self._analysis_report=None
            self.import_error=''

    def set_settings(self, update: dict):
        valid = {'game_dir', 'docs_dir', 'tree_id', 'snapshot_path', 'country', 'language', 'ui_language', 'mods'}
        if not isinstance(update, dict) or any(k not in valid for k in update):
            raise ValueError('Impostazioni non riconosciute')
        if any(not isinstance(v, str) or len(v) > 2048 for v in update.values()):
            raise ValueError('Le impostazioni devono contenere solo stringhe')
        if 'country' in update and (len(update['country']) != 3 or not update['country'].isalpha()):
            raise ValueError('Codice paese non valido (esempio AUS)')
        if 'language' in update and update['language'] not in ('italian', 'english'):
            raise ValueError('Lingua non supportata')
        if 'ui_language' in update and update['ui_language'] not in ('english','italian'):
            raise ValueError('Unsupported interface language')
        if 'mods' in update and update['mods'] not in ('auto', 'off'):
            raise ValueError('Impostazione mod non valida')
        with self.lock:
            self.settings.update(update)
            if set(update)-{'ui_language'}:self.refresh()
            SETTINGS.parent.mkdir(exist_ok=True, parents=True)
            SETTINGS.write_text(json.dumps(self.settings, indent=2, ensure_ascii=False), encoding='utf-8')

    def overview(self):
        with self.lock:
            return {
                'application':'hoi4companion',
                'settings': self.settings.copy(), 'demo': self.demo,
                'parser': self.parser_info.copy(),
                'warnings': self.warnings[:70],
                'dlcs': self.dlcs.to_json() if self.dlcs else None,
                'mods': getattr(self, 'mod_inventory', None),
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
            result['visuals'] = getattr(self, 'focus_visuals', {})
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
        if getattr(self,'video_jobs',None):self.video_jobs.close()
        self.live_hub.close()
        self.live_idle()

    def live_state(self, force=False):
        with self.lock:
            path = self.settings['game_dir']
            previous_provider=getattr(self,'live_provider',None)
            topology=self.resolution_index().geography.neighbors if self.live_monitor is None or previous_provider is not None else None
            topology_changed=previous_provider is not None and previous_provider.province_neighbors is not topology
            if self.live_monitor is None or self.live_provider_path != path or topology_changed:
                if getattr(self, 'live_provider', None) is not None:
                    self.live_provider.close()
                self.live_provider = NativeProvider(Path(path) / 'hoi4.exe',
                    province_neighbors=topology)
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
                        overview=sample['country_overview']
                        section=overview.get('fortifications')
                        if isinstance(section,dict):
                            from .fortifications import enrich_borders
                            overview={**overview,'fortifications':enrich_borders(
                                section,self.resolution_index().geography.neighbors if section.get('status')=='observed' else {},sample['country'])}
                        sample['country_overview']=localise_country(overview, labels)
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
                    'active_guide_id':getattr(self,'active_guide_id',None),
                    'active_guide_revision':getattr(self,'active_guide_revision',0),
                    'overlay':self.hud_status.read(getattr(self,'active_guide_id',None),self._server_identity()) if getattr(self,'hud_status',None) else None,
                    'guide_states': states,
                    'guide_started': {key: self._progress_store().get(key)['started']
                                      for key in self._progress_store().metadata()},
                    'guide_baselines': {key: self._progress_store().get(key).get('baselines', {})
                                        for key in self._progress_store().metadata()},
                    'guide_runs': self._progress_store().metadata(),
                    'guide_server_id':self._server_identity(),
                    'guide_storage':self._storage_status(),
                    'guide_revision': getattr(self,'guide_revision',0)}

    def active_guide(self,update=None):
        """One selection shared by browser pages and native HUD, not progress."""
        with self.lock:
            if update is not None:
                if not isinstance(update,dict) or set(update)!={'guide_id'} or not isinstance(update['guide_id'],str):
                    raise ValueError('Selezione guida non valida')
                definition=load_guide(update['guide_id'])
                if definition['id']!=getattr(self,'active_guide_id',None):
                    path=SETTINGS.with_name('active-guide.json');temporary=path.with_suffix('.json.tmp')
                    path.parent.mkdir(parents=True,exist_ok=True)
                    temporary.write_text(json.dumps({'guide_id':definition['id']}),encoding='utf-8')
                    temporary.replace(path)
                    self.active_guide_id=definition['id']
                    self.active_guide_revision=getattr(self,'active_guide_revision',0)+1
            else:
                definition=load_guide(getattr(self,'active_guide_id',None))
            return {'schema_version':VERSION,'guide_id':definition['id'],'title':definition['title'],
                    'revision':getattr(self,'active_guide_revision',0),'server_id':self._server_identity()}

    def _progress_store(self):
        if not hasattr(self, 'guide_runs'):
            self.guide_runs = RunStore()
        return self.guide_runs

    def _server_identity(self):
        if not hasattr(self,'guide_server_id'):self.guide_server_id=uuid4().hex
        return self.guide_server_id

    def _storage_status(self):
        error=getattr(self,'progress_error','')
        return {'status':'unavailable' if error else 'persistent' if self._progress_store()._database else 'memory',
                'error':error}

    def _write_guard(self,update,current):
        if getattr(self,'progress_error',''):raise ValueError(self.progress_error)
        if current.get('campaign_id'):
            from .campaign import observed_id
            cached=getattr(self,'live_cache',None) or {}
            sample=(cached.get('sample') or {}) if cached.get('usable') else {}
            if observed_id(sample)!=current['campaign_id'] or sample.get('country')!=current.get('country'):
                raise ValueError('Identità della campagna non verificata: avanzamento sospeso')
        if 'server_id' in update and update['server_id']!=self._server_identity():raise ValueError('Server riavviato: ricarica l’avanzamento prima di confermare')
        if 'expected_run_id' in update and update['expected_run_id']!=current['run_id']:raise ValueError('Campagna o checklist cambiata: ricarica prima di confermare')
        if 'expected_revision' in update and (type(update['expected_revision']) is not int or update['expected_revision']!=current.get('run_revision',0)):
            raise ValueError('Avanzamento aggiornato da un’altra vista: ricarica prima di confermare')
        return {k:v for k,v in update.items() if k not in ('server_id','expected_run_id','expected_revision')}

    def _sync_progress(self):
        runs = self._progress_store()
        if not hasattr(self, 'guide_states'):
            self.guide_states = {}
        for guide_id in runs.metadata():
            checks = runs.get(guide_id)['checks']
            self.guide_states[guide_id] = checks
            if guide_id == getattr(self, 'default_guide_id', None):
                self.guide_checks = checks.copy()
        self._progress_synced=True

    def _bind_progress(self):
        # Consume only witnessed native identity; never infer it from a tag/date.
        cached = getattr(self, 'live_cache', None)
        sample=(cached.get('sample') or {}) if cached and cached.get('usable') else {}
        session=sample.get('session_id')
        if getattr(self,'progress_error',''):return
        from .campaign import observed_id
        from .progress_storage import ProgressStorageError
        try:
            campaign=observed_id(sample)
            changed=(self._progress_store().bind_campaign(campaign,sample.get('country'),session,sample.get('native_tick'))
                     if campaign else self._progress_store().bind(session))
            if changed or not getattr(self,'_progress_synced',False):
                self._sync_progress()
            if changed:
                self.guide_revision=getattr(self,'guide_revision',0)+1
            if session:
                for guide_id in self._progress_store().metadata():
                    try:definition=self._runtime_definition(load_guide(guide_id))
                    except (OSError,ValueError):continue
                    if self._progress_store().observe(definition,cached,allow_research=guide_id==getattr(self,'active_guide_id',None)):
                        self.guide_revision=getattr(self,'guide_revision',0)+1
        except ProgressStorageError as exc:
            self.progress_error=str(exc)
            self.guide_revision=getattr(self,'guide_revision',0)+1

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
                update=self._write_guard(update,runs.get(guide['id']))
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
            return {**runs.get(guide['id'], run_id), 'revision': getattr(self, 'guide_revision', 0),
                    'server_id':self._server_identity(),'history':runs.history(guide['id']),
                    'storage':self._storage_status()}

    def walkthrough_runtime(self, guide_id=None, run_id=None):
        with self.lock:
            state = self.live_state()
            guide = self.walkthrough(guide_id)
            progress = self.walkthrough_progress(guide_id=guide['id'], run_id=run_id)
            return evaluate_guide(guide, state, progress)

    def walkthrough_projection(self,guide_id=None,*,live=True) -> GuideProjection:
        """One coherent Rust decision/state/progress packet for presentation."""
        with self.lock:
            state=self.live_state() if live else {'schema_version':1,'usable':False,'session_id':None,'facts':{}}
            guide=self.walkthrough(guide_id)
            progress=self.walkthrough_progress(guide_id=guide['id'])
            return {'schema_version':1,'engine':'rust-guide-core','server_id':self._server_identity(),
                    'guide_id':guide['id'],'definition_revision':guide['runtime_definition_revision'],
                    'state':state,'progress':progress,'evaluation':evaluate_guide(guide,state,progress)}

    def walkthrough_checks(self, update=None, guide_id=None):
        with self.lock:
            default_id = getattr(self, 'default_guide_id', None) or default_guide_id()
            guide = load_guide(guide_id or default_id)
            self._ensure_progress(guide['id'])
            current = self._progress_store().get(guide['id'])['checks']
            if update is not None:
                self._write_guard({},self._progress_store().get(guide['id']))
                current = update_checks(guide,current,update)
                self._progress_store().update(guide['id'], current,
                                              started={} if update == {'reset': True} else None)
                self._sync_progress()
                self.guide_revision += 1
            return {'checks':current.copy(),'revision':self.guide_revision}

    def walkthrough(self, guide_id=None):
        with self.lock:
            definition = self._runtime_definition(load_guide(guide_id or getattr(self, 'default_guide_id', None)))
            if any(step.get('relative_research') or step.get('relative_research_unresolved') or step.get('research_choice') for rows in definition['tracks'].values() for step in rows):
                from .relative_research import project
                self._ensure_progress(definition['id'])
                progress = self._progress_store().get(definition['id'])
                state = getattr(self, 'live_cache', None) or {'schema_version':1,'usable':False,'facts':{}}
                assessment = evaluate_guide(definition, state, progress)
                bindings = progress.get('research_bindings', {}) if assessment['mode']=='live' and assessment['progress_status']=='accepted' else {}
                definition = project(definition, bindings)
            result = enrich_guide(definition, self.trees, self.technologies['technologies'],
                                  self.labels, self.technologies['folders'],
                                  continuous_focuses=getattr(self,'continuous_focuses',{}))
            result['catalog_country'] = self.settings['country'].upper()
            result['demo'] = self.demo
            from .action_resources import enrich_resource_costs
            enrich_resource_costs(result,getattr(self,'operation_entities',[]))
            import hashlib
            result['runtime_definition_revision']=hashlib.sha256(json.dumps(result,sort_keys=True,separators=(',',':'),ensure_ascii=False,allow_nan=False).encode()).hexdigest()
            result['runtime_engine']='rust-guide-core'
            return result

    def _runtime_definition(self, definition):
        if any(step.get('research_upgrade') and not step.get('relative_research')
               for rows in definition['tracks'].values() for step in rows):
            from .relative_research import adapt_legacy
            if getattr(self,'_relative_legacy_index',None) is None:
                self._relative_legacy_index = {row['id']:row for row in self.resolution_index().rows(('technology',))}
                self._relative_legacy_cache = {}
            definition=adapt_legacy(definition,self._relative_legacy_index,self._relative_legacy_cache)
        continuous=getattr(self,'continuous_focuses',{})
        if any(step['action']['kind']=='select_focus' and step['action']['entity']['id'] in continuous
               and step['action'].get('params',{}).get('focus_mode')!='continuous'
               for rows in definition['tracks'].values() for step in rows):
            from copy import deepcopy
            definition=deepcopy(definition)
            for rows in definition['tracks'].values():
                for step in rows:
                    if step['action']['kind']=='select_focus' and step['action']['entity']['id'] in continuous:
                        step['action'].setdefault('params',{})['focus_mode']='continuous'
        from .campaign_conditions import timing_condition
        timing_index=self.resolution_index() if hasattr(self,'operation_entities') and any(step['action'].get('params',{}).get('timing') for rows in definition['tracks'].values() for step in rows) else None
        known_timing=any(timing_condition(step['action'].get('params',{}).get('timing'),timing_index,definition.get('applies_to',{}).get('country'),
                            scope=definition.get('applies_to',{}).get('tree_id'),currency=step['action'].get('params',{}).get('cost_currency'))
                         for rows in definition['tracks'].values() for step in rows)
        word_reminders=(any(source.get('type')=='file' for source in definition.get('sources',[]))
                        and (any('[Table row]' in source.get('text','') for source in definition.get('sources',[]))
                             or any(step.get('tracking')=='reminder' for rows in definition['tracks'].values() for step in rows)))
        production_orders=any(step['action'].get('params',{}).get('category')=='production' for rows in definition['tracks'].values() for step in rows)
        if (definition.get('dependency_policy')=='explicit' and hasattr(self,'operation_entities')
                and (word_reminders or known_timing or production_orders)):
            # Old Word imports and exact supported timings use the same compiler
            # as new previews. load_guide deliberately returns independent copies:
            # key by content and catalog identity, not by Python object identity.
            # Keep authored object order (including multi-resource explanations).
            index=self.resolution_index()
            cache=getattr(self,'_informal_operation_cache',None)
            if cache is None:self._informal_operation_cache=cache={}
            key=definition.get('id')
            previous=cache.get(key)
            import hashlib
            signature=hashlib.sha256(json.dumps(definition,ensure_ascii=False,separators=(',',':'),allow_nan=False).encode()).hexdigest()
            if previous and previous[0]==signature and previous[1] is index:
                return previous[2]
            from copy import deepcopy
            from .informal_operations import promote_operations
            from .operation_import import bind_operations
            from .guide import normalise_guide
            adapted=deepcopy(definition)
            if promote_operations(adapted,index):
                bind_operations(adapted,index)
                adapted=normalise_guide(adapted)
            else:adapted=definition
            if len(cache)>=256:cache.clear()
            cache[key]=(signature,index,adapted)
            return adapted
        return definition

    def associations(self,guide_id=None,update=None):
        from .guide_associations import read_associations,change_association
        with self.lock:
            if update is not None:
                if not isinstance(update,dict) or set(update)!={'guide_id','key','expected','entity'}:
                    raise ValueError('Correzione associazione non valida')
                guide_id=update['guide_id']
            source=load_guide(guide_id or getattr(self,'default_guide_id',None),associations=False)
            if update is not None:
                change_association(source,update['key'],update['expected'],update['entity'],self.importer().catalog())
                self.guide_revision=getattr(self,'guide_revision',0)+1
            return {'guide':self.walkthrough(source['id']),
                    'changes':read_associations(source['id'])['changes'],
                    'revision':getattr(self,'guide_revision',0)}

    def script_analysis(self, refresh=False):
        from .script_analysis import analyze_installation, analyze_documents
        # Expensive diagnostics never hold the lock shared with live/SSE APIs.
        with self._analysis_lock:
            with self.lock:
                if self._analysis_report is not None and not refresh:
                    return self._analysis_report
                generation=self._analysis_generation
                game,mods=getattr(self,'game_root',None),list(getattr(self,'mod_roots',[]))
                labels,reference=self.labels,getattr(self,'reference_labels',{})
                demo=self.demo
            if demo:
                from .jomini_parser import parse_file
                report=analyze_documents([('common/national_focus/demo.txt',parse_file(DEMO))],labels,reference)
                report['demo']=True
            else:report=analyze_installation(game,mods,labels,reference)
            with self.lock:
                if generation!=self._analysis_generation:
                    raise ValueError('Cataloghi ricaricati durante l’analisi: ripeti il controllo')
                from .source_errors import record_source_errors
                report=record_source_errors(report,SETTINGS.parent/'logs'/'source-errors.json')
                self._analysis_report=report
            return report

    def importer(self):
        with self.lock:
            if not hasattr(self, '_importer'):
                self._importer = Importer(self.import_catalog, research_context=self.research_start,
                                         catalog_export=self.export_catalog,resolution_index=self.resolution_index,
                                         catalog_lock=self.lock,compatibility=self.guide_availability)
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
        return self.resolution_index().export()

    def guide_compatibility(self):
        if getattr(self,'demo',True) or getattr(self,'dlcs',None) is None:return None
        with self.lock:
            if getattr(self,'_compatibility_engine',None) is None:
                from .guide_compatibility import GuideCompatibility
                self._compatibility_engine=GuideCompatibility(self)
            return self._compatibility_engine

    def guide_list(self):
        rows=list_guides();engine=self.guide_compatibility()
        for row in rows:
            row['compatibility']=engine.assess(load_guide(row['id'])) if engine else {'blocked':False,'status':'unknown','requirements':[],'unknown':True}
        return rows

    def guide_availability(self,guide):
        engine=self.guide_compatibility()
        return engine.assess(guide) if engine else {'blocked':False,'status':'unknown','requirements':[],'unknown':True}

    def word_inventory(self,source):
        from copy import deepcopy
        from .word_guides import list_documents, prepare_document, _archive
        from .guide_authoring import compile_document
        import hashlib
        source.seek(0);hasher=hashlib.sha256()
        for chunk in iter(lambda:source.read(1024*1024),b''):hasher.update(chunk)
        digest=hasher.hexdigest();source.seek(0)
        with self.lock:
            cache_key=(digest,getattr(self,'_analysis_generation',0))
            cached=getattr(self,'_word_compatibility_cache',{}).get(cache_key)
            if cached is not None:return deepcopy(cached)
        owner=getattr(self,'_catalog_generation',None)
        if owner is not None:
            from .word_guides import _native_source
            with _native_source(source) as path:result=owner.query('word_inventory',path=str(path))
        else:
            result=list_documents(source);result['compatibility']={}
            engine=self.guide_compatibility()
            if engine and not result['single_document']:
                source.seek(0)
                with _archive(source) as archive:sizes={i.filename:i.file_size for i in archive.infolist()}
                budget=64_000_000;count=0
                country_docs=[p for p in result['documents'] if '/country guides/' in ('/'+p).casefold() and '/tools/' not in p.casefold()]
                for entry in country_docs or result['documents']:
                    budget-=sizes.get(entry,0);count+=1
                    if budget<0 or count>128:
                        result['compatibility'][entry]={'blocked':False,'status':'unknown','requirements':[],'unknown':True};continue
                    try:
                        source.seek(0);packet=prepare_document(source,entry)
                        result['compatibility'][entry]=engine.assess(compile_document(packet['document'])['guide'])
                    except (ValueError,OSError):
                        result['compatibility'][entry]={'blocked':False,'status':'unknown','requirements':[],'unknown':True}
        with self.lock:
            cache=getattr(self,'_word_compatibility_cache',{})
            if len(cache)>=4:cache.pop(next(iter(cache)))
            cache[cache_key]=deepcopy(result);self._word_compatibility_cache=cache
        return result

    def resolution_index(self):
        from .resolution import CatalogIndex
        with self.lock:
            generation=getattr(self,'_analysis_generation',0)
            if getattr(self,'_resolution_generation',None)!=generation:
                owner=getattr(self,'_catalog_generation',None)
                if owner is not None:
                    data=owner.query('entities')
                    library=getattr(self,'reference_library',None)
                    self._resolution_index=CatalogIndex(data['entries'],country_names=data['country_names'],adjectives=data['adjectives'],
                        reference_designs=library.manifest() if library else None)
                    self._resolution_generation=generation
                    return self._resolution_index
                entries=catalog_entities(self.trees,self.technologies['technologies'],self.labels,
                    getattr(self,'reference_labels',{}),self.technologies.get('excluded_technologies',()),
                    continuous_focuses=getattr(self,'continuous_focuses',{}))+getattr(self,'operation_entities',[])
                excluded={(row['scope'],row['id']) for row in getattr(self,'excluded_focuses',[])}
                entries=[row for row in entries if row['kind']!='focus' or (row.get('scope'),row['id']) not in excluded]
                from .scripted_names import NameExpander
                expander=NameExpander(getattr(self,'scripted_names',{}),getattr(self,'reference_labels',{}))
                for entry in entries:
                    alternatives=expander.expand(entry['name'],countries=entry.get('countries',()))
                    if alternatives:entry['aliases']=list(dict.fromkeys([*entry.get('aliases',[]),*alternatives]))
                country_names={}
                for label,text in getattr(self,'reference_labels',{}).items():
                    match=re.fullmatch(r'([A-Z0-9]{3})(?:_(?:democratic|fascism|neutrality|communism))?',label)
                    if match and isinstance(text,str) and '[' not in text and '$' not in text:
                        tag=match[1];names=country_names.setdefault(tag,{tag})
                        names.add(text)
                        words=re.findall(r'[A-Za-z]+',text)
                        if 2<=len(words)<=4:names.add(''.join(w[0] for w in words).upper())
                adjectives=[text for key,text in getattr(self,'reference_labels',{}).items() if key.endswith('_ADJ')]
                library=getattr(self,'reference_library',None)
                self._resolution_index=CatalogIndex(entries,country_names=country_names,adjectives=adjectives,
                                                    reference_designs=library.manifest() if library else None)
                self._resolution_generation=generation
            return self._resolution_index

    def export_catalog(self, entries):
        from .catalog_export import for_ai
        with self.lock:
            return for_ai(entries,getattr(self,'reference_labels',{}),self.labels)

    def reference_inventory(self):
        from .reference_presets import PresetInventory
        with self.lock:
            data = self.reference_library.manifest()
            if not hasattr(self, '_preset_inventory'):
                self._preset_inventory = PresetInventory()
            presets = self._preset_inventory.inspect(getattr(self, 'settings', {}).get('docs_dir'), data['designs'])
            return {**data, 'game_presets': presets}

    def technology_catalog(self):
        with self.lock:
            folders = []
            for folder in self.technologies['folders'].values():
                item = {k: v for k, v in folder.items() if k != 'technologies'} | {'count': len(folder['technologies'])}
                if folder.get('tab'):
                    item['tab'] = {**folder['tab'], 'sprite': self.sprites.resolve(folder['tab']['sprite_name']) if self.sprites else None,
                                   'inactive_sprite': self.sprites.resolve(folder['tab']['sprite_name'],2) if self.sprites else None}
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
            self.send_header('Content-Security-Policy', "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'")
            self.end_headers()
            self.wfile.write(data)

        def json(self, data, status=200):
            from .i18n import localize_payload
            data=localize_payload(data,getattr(store,'settings',{}).get('ui_language','english'))
            self._send(status, json.dumps(data, ensure_ascii=False).encode('utf-8'), 'application/json; charset=utf-8')

        def _discard_rejected_body(self):
            # Closing a socket with an unread POST body can reset it on Windows
            # before the client receives the 403. Never parse or execute it, and
            # never drain an unlimited upload or wait indefinitely for its body.
            if self.headers.get('Transfer-Encoding'):return
            try:length=int(self.headers.get('Content-Length','0'))
            except ValueError:return
            if not 0<length<=16_384:return
            timeout=self.connection.gettimeout()
            try:
                self.connection.settimeout(.25)
                self.rfile.read(length)
            except OSError:pass
            finally:self.connection.settimeout(timeout)

        def _guide_media(self,query):
            guide=store.walkthrough(query.get('id',[''])[0])
            step=next((s for rows in guide['tracks'].values() for s in rows if s['key']==query.get('key',[''])[0]),None)
            if not step:raise ValueError('Istruzione non trovata')
            return guide,step,media_references(guide,step)

        def _send_clip(self,path):
            # A browser can seek within the locally generated clip. Accept one
            # well-formed byte range only; paths never come from the request.
            import re
            size=path.stat().st_size;start,end=0,size-1
            header=self.headers.get('Range')
            if header:
                match=re.fullmatch(r'bytes=(\d*)-(\d*)',header.strip())
                if not match or not (match[1] or match[2]):
                    self.send_response(416);self.send_header('Content-Range',f'bytes */{size}');self.send_header('Content-Length','0');self.end_headers();return
                if not match[1]:
                    suffix=int(match[2]);start=max(0,size-suffix)
                else:
                    start=int(match[1]);end=min(size-1,int(match[2])) if match[2] else size-1
                if start>=size or start>end:
                    self.send_response(416);self.send_header('Content-Range',f'bytes */{size}');self.send_header('Content-Length','0');self.end_headers();return
            self.send_response(206 if header else 200)
            self.send_header('Content-Type','video/mp4');self.send_header('Accept-Ranges','bytes')
            self.send_header('Content-Length',str(end-start+1));self.send_header('X-Content-Type-Options','nosniff')
            self.send_header('Cache-Control','no-store')
            if header:self.send_header('Content-Range',f'bytes {start}-{end}/{size}')
            self.end_headers()
            with path.open('rb') as stream:
                stream.seek(start);remaining=end-start+1
                while remaining:
                    data=stream.read(min(65536,remaining))
                    if not data:break
                    self.wfile.write(data);remaining-=len(data)

        def do_GET(self):
            url = urlparse(self.path)
            if url.path in ('/api/reference-library','/api/reference-library/image'):
                try:
                    if url.path.endswith('/image'):
                        raw,mime=store.reference_library.image(parse_qs(url.query).get('id',[''])[0])
                        self._send(200,raw,mime)
                    else:self.json(store.reference_inventory())
                except (ValueError,OSError) as exc:self.json({'error':str(exc)},404)
                return
            if url.path == '/api/import/frame':
                query = parse_qs(url.query)
                try:
                    # GuideFrames only opens a registered local YouTube source.
                    # No arbitrary paths or remote download in the preview endpoint.
                    path = store.guide_frames.frame(query.get('url', [''])[0], float(query.get('seconds', [''])[0]))
                    self._send(200, path.read_bytes(), 'image/jpeg')
                except (ValueError, OSError) as exc:
                    self.json({'error': str(exc)}, 404)
                return
            if url.path in ('/api/guide/media','/api/guide/frame','/api/guide/clip'):
                query=parse_qs(url.query,keep_blank_values=True)
                try:
                    guide,step,media=self._guide_media(query)
                    if url.path.endswith('/media'):
                        availability={}
                        for ref in media['references']:
                            source=ref['url']
                            if source not in availability:
                                try:
                                    store.guide_frames.source(source)
                                    meta=store.guide_frames.metadata(source)
                                    availability[source]=(True,meta.get('duration'))
                                except (ValueError,OSError):availability[source]=(False,None)
                            available,total=availability[source];ref['source_available']=available
                            if type(total) in (int,float) and total>0:
                                ref['time_available']=ref['seconds']<total
                                ref['clip_end']=min(ref['clip_end'],total) if ref['time_available'] else None
                        self.json(media);return
                    if 'ref' in query:
                        supplied=query['ref'][0]
                        reference=next((r for r in media['references'] if r['id']==supplied),None)
                    else:
                        reference=frame_reference(guide,step)
                    if not reference:raise ValueError('Questa istruzione non contiene un fotogramma temporizzato della fonte')
                    if url.path.endswith('/clip'):
                        self._send_clip(store.guide_frames.clip(reference['url'],reference['seconds']))
                    else:
                        path=store.guide_frames.frame(reference['url'],reference['seconds'])
                        self._send(200,path.read_bytes(),'image/jpeg')
                except (ValueError,OSError) as exc:self.json({'error':str(exc)},404)
                return
            if url.path in ('/api/import/video/status', '/api/import/video/file'):
                query=parse_qs(url.query)
                try:
                    jobs=store.video_jobs
                    key=query.get('id',[''])[0]
                    if url.path.endswith('/status'):
                        self.json(jobs.status(key))
                    else:
                        path=jobs.file(key,query.get('name',[''])[0])
                        self.send_response(200)
                        self.send_header('Content-Type',mimetypes.guess_type(path.name)[0] or 'application/octet-stream')
                        self.send_header('Content-Length',str(path.stat().st_size))
                        self.send_header('Content-Disposition','attachment; filename="'+path.name+'"')
                        self.send_header('X-Content-Type-Options','nosniff')
                        self.end_headers()
                        with path.open('rb') as stream:
                            import shutil
                            shutil.copyfileobj(stream,self.wfile,1024*1024)
                except (ValueError,OSError) as exc:self.json({'error':str(exc)},400)
                return
            if url.path == '/api/script-analysis':
                try:self.json(store.script_analysis(refresh=parse_qs(url.query).get('refresh')==['1']))
                except (OSError,ValueError) as exc:self.json({'error':str(exc)},400)
                except RuntimeError as exc:self.json({'error':str(exc)},503)
                return
            if url.path == '/api/script-rules':
                from .script_analysis import RuleBook
                query=parse_qs(url.query);kind=query.get('kind',['trigger'])[0];name=query.get('name',[''])[0]
                if kind not in ('trigger','effect') or not 1<=len(name)<=160:
                    self.json({'error':'Indica trigger/effect e un nome valido'},400);return
                try:self.json(RuleBook.load().describe(kind,name))
                except (OSError,ValueError) as exc:self.json({'error':str(exc)},400)
                return
            if url.path == '/api/import/status':
                self.json({**store.importer().status(), 'youtube_available': bool(ytdlp_path())})
                return
            if url.path == '/api/import/prompt':
                self.json({'prompt': store.importer().prompt()})
                return
            if url.path == '/api/import/catalog':
                mode=parse_qs(url.query).get('export',[''])[0]
                if mode not in ('','ai'):
                    self.json({'error':'Formato export catalogo sconosciuto'},400)
                    return
                importer=store.importer()
                self.json({'entities': importer.ai_catalog() if mode=='ai' else importer.catalog(), 'demo': store.demo,
                           **({'localization_policy':'local_lookup','alias_policy':'english_reference_only'} if mode=='ai' else {})})
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
                                from .i18n import localize_payload
                                payload = json.dumps(localize_payload(event.value,getattr(store,'settings',{}).get('ui_language','english')), ensure_ascii=False, separators=(',', ':'))
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
                    self.json({'schema_version': VERSION, 'guides': store.guide_list()})
                except (OSError, ValueError) as exc:
                    self.json({'error': str(exc)}, 400)
                return
            if url.path == '/api/guide/active':
                try:self.json(store.active_guide())
                except (OSError,ValueError) as exc:self.json({'error':str(exc)},400)
                return
            if url.path == '/api/guide':
                try:
                    self.json(store.walkthrough(parse_qs(url.query).get('id', [None])[0]))
                except (OSError, ValueError) as exc:
                    self.json({'error': str(exc)}, 400)
                return
            if url.path == '/api/guide/associations':
                try:self.json(store.associations(parse_qs(url.query).get('id',[None])[0]))
                except (OSError,ValueError) as exc:self.json({'error':str(exc)},400)
                return
            if url.path == '/api/guide/checks':
                try:
                    self.json(store.walkthrough_checks(guide_id=parse_qs(url.query).get('id', [None])[0]))
                except (OSError, ValueError) as exc:
                    self.json({'error': str(exc)}, 400)
                return
            if url.path == '/api/guide/projection':
                try:
                    query=parse_qs(url.query)
                    if query.get('live',['1'])[0] not in ('0','1'):raise ValueError('Invalid projection mode')
                    self.json(store.walkthrough_projection(query.get('id',[None])[0],live=query.get('live')!=['0']))
                except (OSError,ValueError) as exc:self.json({'error':str(exc)},400)
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
                      '/analysis':'analysis.html','/analysis.js':'analysis.js','/analysis.css':'analysis.css',
                      '/associations':'associations.html','/associations.js':'associations.js',
                      '/importer': 'importer.html', '/importer.js': 'importer.js',
                      '/importer-model.js': 'importer-model.js', '/importer.css': 'importer.css',
                      '/contracts.js': 'contracts.js',
                      '/focus-criteria.js':'focus-criteria.js',
                      '/country': 'country.html', '/country.js': 'country.js',
                      '/country.css': 'country.css', '/country-model.js': 'country-model.js',
                      '/walkthrough': 'guide.html', '/hud': 'guide.html', '/guide-model.js': 'guide-model.js',
                      '/guide.js': 'guide.js', '/guide.css': 'guide.css',
                      '/guide-references.js': 'guide-references.js',
                      '/guide-runtime.js': 'guide-runtime.js',
                      '/operational-summary.js':'operational-summary.js',
                      '/action-plan.js':'action-plan.js',
                      '/focus-layout.js':'focus-layout.js',
                      '/production-progress.js': 'production-progress.js',
                      '/design-matching.js': 'design-matching.js',
                      '/guide-presentation.js': 'guide-presentation.js', '/guide-session.js': 'guide-session.js',
                      '/guide-availability.js': 'guide-availability.js',
                      '/guide-assist.js': 'guide-assist.js', '/guide-assist.css': 'guide-assist.css',
                      '/live-state.js': 'live-state.js', '/live-state.css': 'live-state.css', '/live-model.js': 'live-model.js',
                      '/technologies': 'technologies.html', '/technologies.js': 'technologies.js', '/technologies.css': 'technologies.css',
                      '/i18n.js':'i18n.js', '/theme.css':'theme.css', '/explorer.js': 'explorer.js', '/style.css': 'style.css', '/game.css': 'game.css'}
            file = static.get(url.path)
            if file:
                data = (WEB / file).read_bytes()
                from .i18n import localize_html, catalog
                if file.endswith('.html'):data=localize_html(data.decode('utf-8'),getattr(store,'settings',{}).get('ui_language','english')).encode('utf-8')
                elif file=='i18n.js':data=('globalThis.HOI_TRANSLATIONS='+json.dumps(catalog(),ensure_ascii=False)+';\n').encode('utf-8')+data
                mime = mimetypes.guess_type(file)[0] or 'application/octet-stream'
                if mime.startswith('text/') or 'javascript' in mime:
                    mime += '; charset=utf-8'
                self._send(200, data, mime)
            else:
                self.json({'error': 'Not found'}, 404)

        def do_POST(self):
            url = urlparse(self.path)
            import_paths = {'/api/import/source', '/api/import/packet', '/api/import/extract', '/api/import/preview', '/api/import/install', '/api/import/document'}
            if url.path not in ('/api/config', '/api/refresh', '/api/guide/active', '/api/guide/checks', '/api/guide/progress','/api/guide/associations','/api/import/video','/api/import/word','/api/import/templates') and url.path not in import_paths:
                self.json({'error': 'Not found'}, 404)
                return
            # Blocca richieste cross-origin da siti terzi verso il servizio locale.
            origin = self.headers.get('Origin', '')
            host = self.headers.get('Host', '')
            if (origin and urlparse(origin).netloc != host) or self.headers.get('Sec-Fetch-Site') == 'cross-site':
                self._discard_rejected_body()
                self.json({'error': 'Origine non autorizzata'}, 403)
                return
            if url.path in ('/api/import/word','/api/import/templates'):
                import tempfile
                from .word_guides import list_documents, prepare_document
                try:
                    length = int(self.headers.get('Content-Length', '0'))
                    if self.headers.get('Content-Type', '').split(';')[0] != 'application/octet-stream':
                        raise ValueError('Carica un documento Word o archivio ZIP')
                    if length < 1 or length > 200_000_000:
                        raise ValueError('File vuoto o superiore a 200 MB')
                    with tempfile.TemporaryFile() as source:
                        remaining = length
                        while remaining:
                            chunk = self.rfile.read(min(1024 * 1024, remaining))
                            if not chunk: raise ValueError('Caricamento incompleto')
                            source.write(chunk); remaining -= len(chunk)
                        source.seek(0)
                        query = parse_qs(url.query, keep_blank_values=True)
                        entry = query.get('entry', [None])[0]
                        context_entry = query.get('context', [None])[0]
                        if url.path=='/api/import/templates':
                            with store.lock:
                                result=store.reference_library.import_archive(source)
                                store._resolution_generation=None
                        else:result = prepare_document(source, entry, context_entry) if entry is not None or query.get('prepare') == ['1'] else store.word_inventory(source)
                    self.json(result)
                except (ValueError, OSError) as exc:
                    self.json({'error': str(exc)}, 400)
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
                if url.path=='/api/import/video':
                    if (not isinstance(update,dict) or 'url' not in update or set(update)-{'url','refresh_source'} or
                            not isinstance(update['url'],str) or type(update.get('refresh_source',False)) is not bool):
                        raise ValueError('Indica il link a un video YouTube')
                    self.json(store.video_jobs.start(update['url'],refresh_source=update.get('refresh_source',False)))
                    return
                if url.path=='/api/guide/associations':
                    self.json(store.associations(update=update))
                    return
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
                if url.path == '/api/guide/active':
                    self.json(store.active_guide(update))
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
