from unittest import TestCase
from copy import deepcopy
from hoi4companion.backend_core import _request
from hoi4companion.resolution import CatalogIndex
from hoi4companion._reference_designs_reference import design_matches,bindings,selector,attach_images

class NativeReferenceIdentityTests(TestCase):
    def test_design_abbreviations_follow_the_catalog_vocabulary(self):
        from unittest.mock import patch
        from hoi4companion.research_upgrades import guide_language
        vocabulary=deepcopy(guide_language());vocabulary['abbreviations']['designs']['ZX']='Fixture tank'
        with patch('hoi4companion.research_upgrades.guide_language',return_value=vocabulary):
            index=CatalogIndex([{'kind':'equipment','id':'tank','name':'Tank'}])
        library={'designs':[{'id':'d','name':'ZX-I1','equipment_id':'tank','category':'land','modules':{},'upgrades':{}}]}
        guide={'tracks':{'operations':[{'key':'d','action':{'kind':'manual','params':{'instructions':['Design Fixture-I1']}}}]}}
        result=_request(0,'reference_bindings',guide=guide,library=library,**index.native_import_args())
        self.assertEqual(result['fixturei1'][0]['reference']['id'],'d')

    def test_recipe_qualifiers_declared_upgrades_and_images_keep_identity(self):
        designs=[{'id':'one','name':'T-I 1 (0/4/SL)','equipment_id':'tank','category':'land','modules':{'gun':'cannon'},'upgrades':{'tank_nsb_engine_upgrade':'0','tank_nsb_armor_upgrade':'4'}},
                 {'id':'two','name':'T-I 1 (2/4/SL)','equipment_id':'tank','category':'land','modules':{'gun':'cannon'},'upgrades':{'tank_nsb_engine_upgrade':'2','tank_nsb_armor_upgrade':'4'}},
                 {'id':'plus','name':'F1+','equipment_id':'plane','category':'air','modules':{'gun':'hmg'},'upgrades':{}}]
        library={'designs':designs,'images':[{'id':'image','name':'T-I 1 SL'}]}
        index=CatalogIndex([{'kind':'equipment','id':key,'name':key,'equipment_ids':[key]} for key in ('tank','plane')],reference_designs=library)
        guide={'tracks':{'operations':[{'key':'design','action':{'kind':'manual','params':{'instructions':['Design T-I1 (0/4)','Produce F1+','Refer to T-I 1 SL']}}}]}}
        for query in ('T-I1','T-I (0/4/SL)','T-I 1 (2/4/SL)','F1+','F1'):
            self.assertEqual(_request(0,'reference_matches',query=query,designs=designs),design_matches(query,designs))
        expected=bindings(guide,index)
        self.assertEqual(_request(0,'reference_bindings',guide=guide,library=library,**index.native_import_args()),expected)
        for query in ('T-I1','F1+','F1'):
            self.assertEqual(_request(0,'reference_selector',query=query,library=library,declared=expected,**index.native_import_args()),selector(query,index,expected))
        target=deepcopy(guide);changes=attach_images(target,index)
        self.assertEqual(_request(0,'reference_attach_images',guide=guide,library=library),{'guide':target,'changes':changes})
