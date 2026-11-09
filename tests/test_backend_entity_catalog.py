"""English reference names and progressive lanes are source-derived."""
from pathlib import Path
import unittest
from hoi4companion.clausewitz import parse
from hoi4companion.focus import Tree,Focus
from hoi4companion._guide_importer_reference import catalog_entities
from hoi4companion._catalog_export_reference import for_ai
from hoi4companion.script_document import encode
from hoi4companion.backend_core import _request

class BackendEntityCatalogTests(unittest.TestCase):
    def fixture(self):
        focus=Focus('HUN_focus','Un nome locale','','',0,0,'',10,[],[],False,False,[],source='f.txt',localization_key='LABEL')
        tree=Tree('tree','f.txt',focuses={'HUN_focus':focus},eligible_countries=['HUN'],branch_conditions={'HUN_focus':parse('OR={has_government=neutrality has_government=democratic} arbitrary=yes')})
        layout=lambda n,axis='x':{'id':'air','layout':{'direction':'UP','tree_id':'line','anchor_x':n,'anchor_y':0}}
        technologies={'fighter':{'name':'Caccia','unlocks':[{'kind':'Equipaggiamento','id':'fighter_equipment_1'}],'paths':[{'target':'fighter2'},{'target':'bomber'}],'folders':[layout(1)]},'fighter2':{'name':'Caccia II','folders':[layout(1)]},'bomber':{'name':'Bombardiere','folders':[layout(2)]},'disabled':{'name':'Disabled','catalog_eligible':False}}
        reference={'LABEL':'A [ROOT.GetAdjective] Galicia','HUN_ADJ':'Hungarian','fighter_equipment_1':'Fighter I','ITA_fighter_equipment_1':'C.200 Saetta','fighter':'Basic Fighter','AIR_ALIAS':'Small Aircraft'}
        labels={'LABEL':'Una Galizia','fighter':'Caccia','HUN_ADJ':'Ungherese','ITA_fighter_equipment_1':'Caccia Saetta'}
        return {'tree':tree},technologies,labels,reference

    def test_focus_metadata_national_nicknames_and_aligned_upgrade_paths(self):
        trees,tech,labels,reference=self.fixture()
        headers={key:{'focuses':{k:vars(v) for k,v in tree.focuses.items()},'eligible_countries':tree.eligible_countries,'branch_conditions':{k:encode(v) for k,v in tree.branch_conditions.items()}} for key,tree in trees.items()}
        actual=_request(0,'source_entities',trees=headers,technologies=tech,labels=labels,reference=reference,unavailable=[],continuous={})
        self.assertEqual(actual,catalog_entities(trees,tech,labels,reference))
        self.assertEqual(next(r for r in actual if r['id']=='fighter')['upgrade_successors'],['fighter2'])

    def test_ai_export_keeps_english_aliases_only_and_local_fallback(self):
        entries=[{'kind':'equipment','id':'gun','name':'Rifle','localized_name':'Fucile','aliases':['Rifle','rifle','Small Arms','Fucile','Small Arms'],'country_aliases':{'HUN':['Hungarian Rifle','Fucile']},'geography':{},'production_ui_category':'infantry','repeatable':False}, {'kind':'focus','id':'X_FOCUS','name':'X_FOCUS','localized_name':'Nome locale'}]
        reference={'name':'Rifle','alias':'Small Arms','country':'Hungarian Rifle'}
        self.assertEqual(_request(0,'source_export',entries=entries,reference=reference),for_ai(entries,reference))
