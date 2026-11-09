from copy import deepcopy
from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion.resolution import CatalogIndex
from hoi4companion._guide_preview_reference import preview
from hoi4companion.import_sources import acquire_source
import test_guide_importer as fixtures


class NativeImportPreviewTests(TestCase):
    def test_focus_resolution_preserves_absent_params_and_uses_inferred_tree_scope(self):
        source=acquire_source(text='Take First Focus. Then Next Focus.')
        index=CatalogIndex([{'kind':'focus','id':key,'name':name,'scope':'tree','countries':['TST'],
            'prerequisites':parents,'mutually_exclusive':[]} for key,name,parents in
            [('a','First Focus',[]),('b','Next Focus',[['a']])]])
        guide={'id':'focus','title':'Fixture','applies_to':{'country':'TST'},
               'sources':[{'id':source['id'],'type':'manual'}],'tracks':{'focus':[
            {'key':key,'action':{'kind':'select_focus','entity':{'kind':'focus','id':name}},
             'evidence':[{'source_id':source['id'],'basis':'explicit','quote':quote}]} for key,name,quote in
            [('first','First Focus','Take First Focus.'),('next','Next Focus','Then Next Focus.')] ]}}
        self.assertEqual(_request(0,'import_preview',guide=guide,sources=[source],library=index.reference_designs,**index.native_import_args()),preview(guide,[source],index))
    def test_source_resolution_evidence_and_preview_errors_match(self):
        source=fixtures.source();guide=fixtures.definition(source)
        cases=[(guide,[source],fixtures.ENTITIES)]
        for query in ('Missing technology','Basic Machine Toolz','basic_machine_tools'):
            value=deepcopy(guide);value['tracks']['research'][0]['action']['entity']['id']=query
            cases.append((value,[source],fixtures.ENTITIES))
        for field,value in (('title',''),('id','../bad'),('id',None),('assumptions',[1])):
            row=deepcopy(guide);row[field]=value;cases.append((row,[source],fixtures.ENTITIES))
        row=deepcopy(guide);row['tracks']['research'][0]['evidence'][0]['quote']='Invented observation';cases.append((row,[source],fixtures.ENTITIES))
        row=deepcopy(guide);row['tracks']['research'][0]['evidence']=[];cases.append((row,[source],fixtures.ENTITIES))
        row=deepcopy(source);row['text']='Edited source';cases.append((guide,[row],fixtures.ENTITIES))
        for value,sources,entries in cases:
            index=CatalogIndex(entries)
            with self.subTest(value=value):
                self.assertEqual(_request(0,'import_preview',guide=value,sources=sources,library=index.reference_designs,**index.native_import_args()),preview(value,sources,index))

    def test_structured_building_and_caption_observations_do_not_become_invented_facts(self):
        source=acquire_source(text='Build 2 military factories in Berlin.')
        guide={'id':'build','title':'Build','applies_to':{},'sources':[{'id':source['id'],'type':'manual'}],
               'dependency_policy':'explicit','tracks':{'operations':[{'key':'build','tracking':'reminder',
                'action':{'kind':'manual','params':{'instructions':[source['text']]}},
                'evidence':[{'source_id':source['id'],'basis':'explicit','quote':source['text']}]}]}}
        index=CatalogIndex([{'kind':'building','id':'arms_factory','name':'Military Factory'},
                            {'kind':'state','id':'64','name':'Berlin'}])
        self.assertEqual(_request(0,'import_preview',guide=guide,sources=[source],library=index.reference_designs,**index.native_import_args()),preview(guide,[source],index))
