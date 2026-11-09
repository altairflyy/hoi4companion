"""Read-only native file overlays retain mod precedence and English identities."""
from pathlib import Path
from tempfile import TemporaryDirectory
import json
import unittest
from hoi4companion.backend_core import _request
from hoi4companion._installation_reference import inspect_mods,collect_files,load_localisations
from hoi4companion._dlc_reference import discover_dlcs


class BackendInstallationTests(unittest.TestCase):
    def test_mod_precedence_replace_paths_and_independent_english_reference(self):
        with TemporaryDirectory() as folder:
            game,docs,mod=(Path(folder)/s for s in ('game','docs','mod'))
            docs.mkdir();mod.mkdir();(docs/'dlc_load.json').write_text(json.dumps({'enabled_mods':['one.mod'],'disabled_dlcs':[]}))
            (docs/'one.mod').write_text('name="Translation" path="'+mod.as_posix()+'"')
            for root in (game,mod):
                (root/'common/national_focus').mkdir(parents=True)
                (root/'localisation').mkdir()
            (game/'common/national_focus/base.txt').write_text('x=base')
            (mod/'descriptor.mod').write_text('replace_path="common/national_focus"')
            (mod/'common/national_focus/new.txt').write_text('x=new')
            (game/'localisation/a_l_english.yml').write_text('l_english:\n KEY:0 "The Industry"\n ESC:0 "Line\\n\\\"quote\\\""\n',encoding='utf8')
            (mod/'localisation/z_l_english.yml').write_text('l_english:\n KEY:0 "Industria"\n NEW:0 "New focus"\n',encoding='utf8')
            args={'game':str(game),'docs':str(docs),'mods':[str(mod)]}
            self.assertEqual(_request(0,'installation_mods',**args),inspect_mods(docs))
            files,warnings=collect_files(game,[mod],'common/national_focus','.txt')
            native=_request(0,'installation_files',**args,subdir='common/national_focus',extension='.txt')
            self.assertEqual(native,{'files':list(map(str,files)),'warnings':warnings})
            self.assertEqual(_request(0,'installation_localisations',**args,language='english'),list(load_localisations(game,[mod],'english')))

    def test_dlc_statuses_are_availability_not_import_errors(self):
        with TemporaryDirectory() as folder:
            game,docs=(Path(folder)/s for s in ('game','docs'));docs.mkdir()
            (docs/'dlc_load.json').write_text('{"disabled_dlcs":["DLC/pack/pack.dlc"]}')
            for group in ('dlc','integrated_dlc'):
                directory=game/group/'pack';directory.mkdir(parents=True)
                (directory/'pack.dlc').write_text('name="Pack '+group+'" path="'+group+'/pack"')
            expected=discover_dlcs(game,docs)
            value=_request(0,'installation_dlcs',game=str(game),docs=str(docs))
            self.assertEqual(value,{'items':expected.items,'complete':expected.complete,'warnings':expected.warnings})
