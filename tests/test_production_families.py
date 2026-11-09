import copy,unittest
from hoi4companion.informal_targets import production_command
from hoi4companion.resolution import CatalogIndex
from hoi4companion.operation_import import bind_operations
from hoi4companion.guide_importer import preview
from hoi4companion.guide_runtime import evaluate
from hoi4companion.screen_overlay import targets
from test_operation_import import document
from test_production_remaining import state,line


CATALOG=[{'kind':'equipment','id':root,'name':name,'aliases':aliases,'family_id':root,
          'is_archetype':True,'equipment_ids':[root+'_1',root+'_2'],'production_ui_category':'interface_category_land'}
         for root,name,aliases in [('train','Train',['Trains']),('truck','Truck',[]),('inf','Infantry Equipment',['Infantry Equip']),('sup','Support Equipment',[])]]
CATALOG += [{'kind':'equipment','id':root+'_'+level,'name':name+' '+level,'localized_name':'Local '+name+' '+level,
             'aliases':['National '+name+' '+level],'family_id':root,'equipment_ids':[root+'_'+level],
             'is_archetype':False,'production_ui_category':'interface_category_land'}
            for root,name in [('train','Train'),('truck','Truck'),('inf','Infantry Equipment'),('sup','Support Equipment')] for level in ('1','2')]


class ProductionFamilyTests(unittest.TestCase):
    def test_grouped_numeric_quantity_applies_to_each_preceding_product(self):
        index=CatalogIndex(CATALOG)
        p=production_command('Train, Truck:1, Sup:7, Inf:ALL',index,'TST')
        self.assertEqual([t.get('requested_factories') for t in p['params']['production_targets']],[1,1,7,None])
        self.assertIsNone(p['remainder'])
        self.assertEqual([t['production_selector']['variant_names'] for t in p['params']['production_targets']],[[],[],[],[]])
        uncertain=production_command('Train, Truck:ALL',index,'TST')
        self.assertEqual(len(uncertain['params']['production_targets']),1)
        self.assertIn('Train',uncertain['remainder'])
        self.assertIsNone(production_command('Train, 1',index,'TST'))

    def test_shared_quantity_with_unknown_product_is_preserved_without_losing_known_targets(self):
        p=production_command('Unknown, Train, Truck:2, Sup:7',CatalogIndex(CATALOG),'TST')
        self.assertEqual([x['requested_factories'] for x in p['params']['production_targets']],[2,2,7])
        self.assertIn('Unknown',p['remainder'])

    def test_generic_family_counts_ignore_arbitrary_live_names_and_model_template_stays_exact(self):
        g,src=document({'category':'production','production_targets':[{'equipment_name':'Train','requested_factories':1},
                                  {'equipment_name':'Infantry Equipment','requested_factories':10}]})
        report=preview(g,src,CATALOG);self.assertTrue(report['valid'],report['errors']);g=report['guide']
        s=state([line('t','train_2',1,1),line('i','inf_2',10,3)])
        for row in s['facts']['production.lines']['value']:row['name']='Anything localized / custom'
        self.assertEqual(evaluate(g,s)['steps'][0]['status'],'completed')
        hud=targets(g,evaluate(g,s),CATALOG,{**s,'sample':{'country':'TST'}})
        self.assertTrue(all(x['goal_satisfied'] for x in hud))
        self.assertTrue(all(x['title']!='Anything localized / custom' for x in hud))
        self.assertTrue(all(x['production_selector']['variant_names']==[] for x in hud))
        wrong=copy.deepcopy(s);wrong['facts']['production.lines']['value'][0]['equipment']='unrelated_type'
        self.assertNotEqual(evaluate(g,wrong)['steps'][0]['status'],'completed')

    def test_catalog_type_in_variant_field_does_not_require_a_national_variant_name(self):
        g,src=document({'category':'production','equipment_name':'Infantry Equipment','variant_name':'National Infantry Equipment 2','requested_factories':10})
        report=preview(g,src,CATALOG);self.assertTrue(report['valid'],report['errors'])
        s=state([line('i','inf_2',10,3)]);s['facts']['production.lines']['value'][0]['name']='Renamed rifle'
        self.assertEqual(evaluate(report['guide'],s)['steps'][0]['status'],'completed')
        self.assertEqual(report['guide']['tracks']['operations'][0]['action']['params']['production_selector']['variant_names'],[])

    def test_catalog_family_wins_over_a_reference_recipe_named_like_a_generic_product(self):
        index=CatalogIndex(CATALOG,reference_designs={'designs':[{'id':'generic','name':'Infantry Equipment','equipment_id':'inf_1','category':'land','modules':{},'upgrades':{}}]})
        g,src=document({'category':'production','equipment_name':'Infantry Equipment','requested_factories':10})
        report=preview(g,src,index);self.assertTrue(report['valid'],report['errors'])
        p=report['guide']['tracks']['operations'][0]['action']['params']
        self.assertEqual(p['production_selector']['equipment_ids'],['inf_1','inf_2'])
        self.assertEqual(p['production_selector']['variant_names'],[])
        self.assertNotIn('design_reference',p)

    def test_custom_template_label_is_preserved_but_completion_requires_its_configuration(self):
        index=CatalogIndex(CATALOG,reference_designs={'designs':[{'id':'custom','name':'Model-I 1 (0/4/SL)','equipment_id':'inf_1','category':'land','modules':{},'upgrades':{}}]})
        g,src=document({'category':'production','equipment_name':'Model-I (0/4/SL)','requested_factories':10})
        report=preview(g,src,index);self.assertTrue(report['valid'],report['errors']);g=report['guide']
        s=state([line('i','inf_1',10,3)]);s['facts']['production.lines']['value'][0]['name']='Different model'
        self.assertNotEqual(evaluate(g,s)['steps'][0]['status'],'completed')
        hud=targets(g,evaluate(g,s),index,{**s,'sample':{'country':'TST'}})
        self.assertEqual(hud[0]['title'],'Model-I 1 (0/4/SL)')
        s['facts']['production.lines']['value'][0]['name']='Model-I 1 (0/4/SL)'
        self.assertNotEqual(evaluate(g,s)['steps'][0]['status'],'completed')
        s['facts']['production.lines']['value'][0].update(name='Different national label',design={'status':'observed','modules':{},'upgrades':{}})
        self.assertEqual(evaluate(g,s)['steps'][0]['status'],'completed')

    def test_old_imports_gain_shared_train_target_in_memory_but_editor_changes_are_preserved(self):
        from hoi4companion.informal_operations import promote_operations
        index=CatalogIndex(CATALOG)
        parsed=production_command('Train, Truck:1, Sup:7, Inf:ALL',index,'TST')
        g,src=document({**parsed['params'],'instructions':['Train, Truck:1, Sup:7, Inf:ALL']})
        g['dependency_policy']='explicit'
        g['tracks']['operations'][0]['action']['params']['production_targets'].pop(0)
        bind_operations(g,index);before=copy.deepcopy(g)
        self.assertTrue(promote_operations(g,index));bind_operations(g,index)
        self.assertEqual(len(g['tracks']['operations'][0]['action']['params']['production_targets']),4)
        self.assertEqual(len(before['tracks']['operations'][0]['action']['params']['production_targets']),3)
        frozen=copy.deepcopy(g);self.assertFalse(promote_operations(g,index));self.assertEqual(g,frozen)
        edited=copy.deepcopy(before);edited['tracks']['operations'][0]['action']['params']['production_targets'][0]['requested_factories']=3
        promote_operations(edited,index)
        self.assertEqual(len(edited['tracks']['operations'][0]['action']['params']['production_targets']),3)
        self.assertEqual(edited['tracks']['operations'][0]['action']['params']['production_targets'][0]['requested_factories'],3)

    def test_unknown_custom_model_does_not_get_promoted_to_the_whole_equipment_family(self):
        g,src=document({'category':'production','equipment_name':'Infantry Equipment','variant_name':'My deliberate custom template','requested_factories':10})
        report=preview(g,src,CATALOG);self.assertTrue(report['valid'],report['errors'])
        p=report['guide']['tracks']['operations'][0]['action']['params']
        self.assertEqual(p['production_selector']['variant_names'],['My deliberate custom template'])
        self.assertNotEqual(evaluate(report['guide'],state([line('i','inf_1',10,3)]))['steps'][0]['status'],'completed')

    def test_generated_residual_never_reassigns_removed_quantities(self):
        from hoi4companion.informal_operations import promote_operations
        from hoi4companion.import_sources import acquire_source
        index=CatalogIndex(CATALOG);text='MIL: Mystery, Train:1, Truck:2'
        source=acquire_source(text='[Table row]\n[Column 1]\n'+text+'\n[Column 2]\n[Column 3]\n')
        source['type']='file';evidence=[{'source_id':source['id'],'quote':text,'basis':'explicit'}]
        parsed=production_command(text,index,'TST')
        g={'id':'residual','dependency_policy':'explicit','sources':[source],'applies_to':{'country':'TST'},'tracks':{'operations':[
            {'key':'word-1--operation-3','evidence':copy.deepcopy(evidence),'action':{'kind':'manual','params':{**parsed['params'],'instructions':[text]}}},
            {'key':'word-1--operation-4','tracking':'reminder','evidence':copy.deepcopy(evidence),'action':{'kind':'manual','params':{'category':'checkpoints','instructions':['MIL: Mystery, Train, Truck:2']}}}]}}
        promote_operations(g,index);bind_operations(g,index)
        self.assertEqual(len(g['tracks']['operations']),2)
        residual=g['tracks']['operations'][1]
        self.assertEqual(residual['tracking'],'reminder')
        self.assertEqual(residual['action']['params']['instructions'],['MIL: Mystery'])
        self.assertEqual([p['requested_factories'] for p in g['tracks']['operations'][0]['action']['params']['production_targets']],[1,2])
        saved=copy.deepcopy(g);self.assertFalse(promote_operations(g,index));self.assertEqual(g,saved)

    def test_obsolete_generated_train_remainder_is_removed_but_original_text_is_kept(self):
        from hoi4companion.informal_operations import promote_operations
        from hoi4companion.import_sources import acquire_source
        text='Train, Truck:1';source=acquire_source(text='[Table row]\n[Column 1]\n'+text+'\n[Column 2]\n[Column 3]\n');source['type']='file'
        evidence=[{'source_id':source['id'],'quote':text,'basis':'explicit'}];index=CatalogIndex(CATALOG)
        parsed=production_command(text,index,'TST')
        g={'id':'old','dependency_policy':'explicit','sources':[source],'applies_to':{'country':'TST'},'tracks':{'operations':[
            {'key':'word-1--operation-3','evidence':copy.deepcopy(evidence),'action':{'kind':'manual','params':{**parsed['params'],'instructions':[text]}}},
            {'key':'word-1--operation-4','tracking':'reminder','evidence':copy.deepcopy(evidence),'action':{'kind':'manual','params':{'category':'checkpoints','instructions':['MIL: Train']}}}]}}
        promote_operations(g,index)
        self.assertEqual(len(g['tracks']['operations']),1)
        self.assertEqual(g['sources'][0]['text'],source['text'])
