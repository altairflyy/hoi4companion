import copy
import io
import json
from pathlib import Path
import tempfile
import threading
from threading import RLock
import unittest
from urllib.request import Request,urlopen
from urllib.error import HTTPError
from zipfile import ZipFile

from PIL import Image
from hoi4companion.api import Store,make_handler
from hoi4companion.reference_designs import ReferenceLibrary,bindings,design_matches
from hoi4companion.resolution import CatalogIndex
from hoi4companion.guide_importer import preview
from hoi4companion.guide_runtime import evaluate
import test_informal_operations as operations
from test_production_remaining import line,state
from http.server import ThreadingHTTPServer


def archive():
    raw=io.BytesIO();picture=io.BytesIO();Image.new('RGB',(4,4)).save(picture,format='PNG')
    with ZipFile(raw,'w') as z:
        z.writestr('pack/equipment_designs.txt','''tank_designs={ category=land
equipment={ name="T-I 1 (0/4/SL)" target_variant={ type=medium modules={ gun=cannon } upgrades={ tank_nsb_engine_upgrade=0 tank_nsb_armor_upgrade=4 } } }
equipment={ name="T-I 1 (2/4/SL)" target_variant={ type=medium modules={ gun=cannon } upgrades={ tank_nsb_engine_upgrade=2 tank_nsb_armor_upgrade=4 } } }
} plane_designs={ category=air
equipment={ name="F1" target_variant={ type=plane modules={ gun=mg } } }
equipment={ name="F1+" target_variant={ type=plane modules={ gun=hmg } } }
}''')
        z.writestr('pack/ARMOR/T-I 1 SL.png',picture.getvalue())
        z.writestr('pack/TEMPLATES/OFF.png',picture.getvalue())
    return raw.getvalue()


class ReferenceDesignTests(unittest.TestCase):
    def test_omitted_tier_requires_a_complete_qualifier_and_keeps_ambiguity(self):
        from hoi4companion.reference_designs import design_matches
        rows=[{'name':'Model-I 1 (0/4/SL)'},{'name':'Model-I 1 (2/4/SL)'}]
        self.assertEqual(design_matches('Model-I (0/4/SL)',rows),[rows[0]])
        self.assertEqual(design_matches('Model-I',rows),[])
        rows.append({'name':'Model-I 2 (0/4/SL)'})
        self.assertEqual(len(design_matches('Model-I (0/4/SL)',rows)),2)

    def test_nested_reference_folders_preserve_category_and_warning_tags(self):
        raw=io.BytesIO();picture=io.BytesIO();Image.new('RGB',(2,2)).save(picture,format='PNG')
        with ZipFile(raw,'w') as z:
            for path in ('TEMPLATES & DESIGNS (UPDATED)/ARMOR DESIGNS (UPDATED)/SITUATIONAL/Model.png',
                         'TEMPLATES & DESIGNS (UPDATED)/AIR DESIGNS (UPDATED)/SITUATIONAL/Model.png',
                         'TEMPLATES & DESIGNS (UPDATED)/MULTIPLAYER (OUTDATED)/Squad.png',
                         'custom/unknown/Other.png'):
                z.writestr(path,picture.getvalue())
        with tempfile.TemporaryDirectory() as temp:
            result=ReferenceLibrary(temp).import_archive(io.BytesIO(raw.getvalue()))
        self.assertEqual(len(result['images']),4)
        self.assertEqual([r['category'] for r in result['images']],['armor','air','multiplayer','other'])
        self.assertEqual(result['images'][0]['tags'],['situational'])
        self.assertEqual(result['images'][2]['tags'],['outdated'])
        self.assertEqual(result['images'][3]['section'],'unknown')

    def catalog(self,library):
        return CatalogIndex([{'kind':'equipment','id':key,'name':key,'equipment_ids':[key],'aliases':[]}
                             for key in ('medium','plane')],reference_designs=library.manifest())

    def test_library_persists_structured_recipes_and_reference_images(self):
        with tempfile.TemporaryDirectory() as temp:
            library=ReferenceLibrary(Path(temp)/'references');data=library.import_archive(io.BytesIO(archive()))
            self.assertEqual((len(data['designs']),len(data['images'])),(4,2))
            self.assertEqual(ReferenceLibrary(library.root).manifest(),data)
            raw,mime=library.image(data['images'][0]['id']);self.assertEqual(mime,'image/png')
            self.assertTrue(raw.startswith(b'\x89PNG'))
            for key in ('../manifest.json','x'*64):
                with self.assertRaises(ValueError):library.image(key)

    def test_declared_upgrades_disambiguate_same_name_and_reimport_is_stable(self):
        with tempfile.TemporaryDirectory() as temp:
            library=ReferenceLibrary(temp);library.import_archive(io.BytesIO(archive()));index=self.catalog(library)
            report,sources=operations.InformalOperationsTests().report(['ASAP: Design T-I1 (0/4)\nProduction: T-I1 to 3'],index)
            rows=report['guide']['tracks']['operations'];production=next(r for r in rows if r['action']['params'].get('production_targets'))
            p=production['action']['params']['production_targets'][0]
            self.assertEqual(p['production_selector']['equipment_ids'],['medium'])
            self.assertEqual(p['design_reference']['name'],'T-I 1 (0/4/SL)')
            self.assertIn('image_id',p['design_reference'])
            self.assertEqual(preview(report['guide'],sources,index)['guide'],report['guide'])
            report,_=operations.InformalOperationsTests().report(['Production: T-I1 to 3'],index)
            self.assertEqual(report['guide']['tracks']['operations'][0]['tracking'],'reminder')

    def test_plus_models_do_not_match_plain_model_and_photos_do_not_complete_actions(self):
        with tempfile.TemporaryDirectory() as temp:
            library=ReferenceLibrary(temp);library.import_archive(io.BytesIO(archive()));index=self.catalog(library)
            report,_=operations.InformalOperationsTests().report(['Design F1+\nProduction: F1+ to 3\nRecruit OFF'],index)
            guide=report['guide'];row=next(r for r in guide['tracks']['operations'] if r['action']['params'].get('production_targets'))
            current=state([line('p','plane',3)]);current['facts']['production.lines']['value'][0]['name']='F1'
            value=next(r for r in evaluate(guide,current)['steps'] if r['key']==row['key'])
            self.assertNotEqual(value['status'],'completed')
            ref=next(r for r in guide['tracks']['operations'] if 'Recruit OFF' in r['title'])
            self.assertEqual(ref['tracking'],'reminder');self.assertTrue(ref['action']['params']['reference_images'])

    def test_reference_archive_duplicates_and_bad_images_fail_without_installing(self):
        with tempfile.TemporaryDirectory() as temp:
            library=ReferenceLibrary(temp);before=library.manifest();data=io.BytesIO()
            with ZipFile(data,'w') as z:z.writestr('bad.png',b'not an image')
            with self.assertRaises((ValueError,OSError)):library.import_archive(io.BytesIO(data.getvalue()))
            self.assertEqual(library.manifest(),before)
            with self.assertRaises(ValueError):library.import_archive(io.BytesIO(b'not a zip'))

    def test_http_import_and_image_roundtrip_invalidate_catalog_and_reject_external_origin(self):
        with tempfile.TemporaryDirectory() as temp:
            store=Store.__new__(Store);store.lock=RLock();store.reference_library=ReferenceLibrary(temp);store._resolution_generation=4
            with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
                worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start();base=f'http://127.0.0.1:{server.server_port}'
                try:
                    request=Request(base+'/api/import/templates',data=archive(),headers={'Content-Type':'application/octet-stream'})
                    with urlopen(request) as response:data=json.load(response)
                    self.assertIsNone(store._resolution_generation)
                    with urlopen(base+'/api/reference-library/image?id='+data['images'][0]['id']) as response:self.assertEqual(response.headers['Content-Type'],'image/png')
                    request=Request(base+'/api/import/templates',data=archive(),headers={'Content-Type':'application/octet-stream','Origin':'https://example.invalid'})
                    with self.assertRaises(HTTPError) as error:urlopen(request)
                    self.assertEqual(error.exception.code,403)
                finally:server.shutdown();worker.join()
