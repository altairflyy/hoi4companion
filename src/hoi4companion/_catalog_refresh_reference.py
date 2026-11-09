"""Development oracle for source-cohort parity, never an application fallback."""
from .api import (Path,json,DEMO,SETTINGS,inspect_mods,collect_files,load_localisations,operation_catalog,
                 load_trees,load_continuous_focuses,discover_dlcs,load_scripted_triggers,filter_trees,
                 excluded_focus_branches,load_technologies,SpriteLibrary,load_technology_visuals,load_focus_visuals)

def refresh(self):
    with self.lock:
        self._resolution_generation=None
        self._relative_legacy_index=None
        self._relative_legacy_cache={}
        self.research_start_cache = {}
        config = self.settings
        game = Path(config['game_dir']).expanduser() if config['game_dir'] else None
        mods = []
        warnings = []
        self.dlcs = None
        self.excluded_trees = []
        self.excluded_focuses = []
        self.technology_definitions = {}
        self._compatibility_engine = None
        self._word_compatibility_cache = {}
        self.sprites = None
        self.mod_inventory = {'status': 'off' if config['mods'] == 'off' else 'unavailable',
                              'source': '', 'basis': 'launcher_profile', 'items': [], 'roots': [], 'warnings': []}
        self.technologies = {'folders': {}, 'technologies': {}, 'warnings': [], 'excluded_folders': []}
        self.technology_visuals = {'templates': {}, 'folders': {}}
        self.focus_visuals = {}
        self.operation_entities = []
        self.demo = not bool(game and (game / 'common' / 'national_focus').is_dir())
        if self.demo:
            files = [DEMO]
            labels = json.loads((DEMO.parent / 'demo_labels.json').read_text(encoding='utf-8'))
            reference_labels = labels
            warnings.append('Modalità dimostrativa: nessun file HOI4 reale caricato.')
        else:
            if config['mods'] == 'auto' and config['docs_dir']:
                self.mod_inventory = inspect_mods(Path(config['docs_dir']).expanduser())
                mods = [Path(path) for path in self.mod_inventory['roots']]
                warnings += self.mod_inventory['warnings']
            files, warns = collect_files(game, mods, 'common/national_focus', '.txt')
            warnings += warns
            labels, reference_labels = load_localisations(game, mods, config['language'])
            self.operation_entities, issues = operation_catalog(game,mods,labels,reference_labels)
            warnings += issues
            if not files:
                warnings.append('Nessun file di focus trovato; controlla il percorso di installazione.')
        trees, warns = load_trees(files, labels)
        self.all_trees = dict(trees)
        self.continuous_focuses = {}
        self.scripted_names={}
        if not self.demo:
            continuous_files, issues = collect_files(game, mods, 'common/continuous_focus', '.txt')
            self.continuous_focuses, continuous_warnings = load_continuous_focuses(continuous_files, labels)
            warnings += issues + continuous_warnings
            from .scripted_names import load_names
            self.scripted_names, name_warnings = load_names(game, mods)
            warnings += name_warnings
        warnings += warns
        if not self.demo:
            self.dlcs = discover_dlcs(game, Path(config['docs_dir']).expanduser() if config['docs_dir'] else None)
            warnings += self.dlcs.warnings
            trigger_files, warns = collect_files(game, mods, 'common/scripted_triggers', '.txt')
            warnings += warns
            scripts, warns = load_scripted_triggers(trigger_files)
            warnings += warns
            trees, self.excluded_trees = filter_trees(trees, self.dlcs, scripts)
            from .focus_criteria import expand_tree_rules
            expand_tree_rules(trees,scripts,self.dlcs)
            self.excluded_focuses = excluded_focus_branches(trees,self.dlcs)
            tech_files, warns = collect_files(game, mods, 'common/technologies', '.txt')
            warnings += warns
            tag_files, warns = collect_files(game, mods, 'common/technology_tags', '.txt')
            warnings += warns
            gui_files, warns = collect_files(game, mods, 'interface', '.gui')
            warnings += warns
            focus_gui_files = gui_files
            gui_files = [file for file in gui_files if file.name.lower().startswith('countrytechtree')]
            self.technologies = load_technologies(tech_files, tag_files, labels, self.dlcs, scripts,
                                                  config['country'].upper(), gui_files=gui_files,
                                                  definition_metadata=self.technology_definitions)
            warnings += self.technologies['warnings']
            asset_roots = [game / Path(x['descriptor']).parent for x in self.dlcs.items
                           if x['status'] in ('enabled', 'integrated')]
            self.sprites = SpriteLibrary(game, [*asset_roots, *mods], SETTINGS.parent / 'sprite-cache')
            warnings += self.sprites.warnings
            gfx_files, warns = collect_files(game, mods, 'interface', '.gfx')
            warnings += warns
            self.technology_visuals = load_technology_visuals(gui_files, gfx_files, self.sprites)
            warnings += self.technology_visuals['warnings']
            self.focus_visuals = load_focus_visuals(focus_gui_files, self.sprites)
            warnings += self.focus_visuals['warnings']
        self.warnings = warnings
        self.labels = labels
        self.reference_labels = reference_labels
        self.game_root, self.mod_roots = game, mods
        self.scripts = scripts if not self.demo else {}
        self.trees = trees
        self._analysis_generation = getattr(self,'_analysis_generation',0)+1
        self._analysis_report = None
        self.import_error = ''
        if self.settings['tree_id'] not in trees:
            self.settings['tree_id'] = next(iter(trees), '')

