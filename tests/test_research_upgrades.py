import copy
import unittest

from hoi4companion.guide_importer import preview, catalog_entities
from hoi4companion.guide_runtime import evaluate
from hoi4companion.import_sources import acquire_source


def tech(key, successors=(), **extra):
    return {'kind':'technology','id':key,'name':key.replace('_',' '),'aliases':[],
            'upgrade_successors':list(successors), **extra}


CATALOG = [tech('basic_machine_tools', ['improved_machine_tools']),
           tech('improved_machine_tools', ['advanced_machine_tools']),
           tech('advanced_machine_tools', ['assembly_line_production']),
           tech('assembly_line_production'), tech('radio', ['improved_radio']),
           tech('improved_radio', ['advanced_radio']), tech('advanced_radio')]


def definition(names, **extra):
    source = acquire_source(text='\n'.join(names))
    guide = {'schema_version':1,'id':'generic-upgrades','title':'Generic upgrades',
             'sources':[{'id':source['id'],'type':'manual'}],
             'tracks':{'research':[{'key':f'r{i}', 'action':{'kind':'research','entity':{'kind':'technology','id':name}},
                 'evidence':[{'source_id':source['id'],'basis':'explicit','quote':name}]} for i,name in enumerate(names)]}, **extra}
    return guide, [source]


class UpgradeTests(unittest.TestCase):
    def test_one_letter_typo_has_precedence_over_new_two_letter_candidates(self):
        from hoi4companion.research_upgrades import typo_matches
        candidates=[tech('one',name='Transport Ship'),tech('two',name='Transport Shift')]
        self.assertEqual([x['id'] for x in typo_matches('Transport Ships',candidates)],['one'])
    def test_confirmed_names_engine_shorthand_and_explicit_up_qualifiers(self):
        catalog=[tech('tank',name='Inter-War Tank Development'),
                 tech('coal',name='Pulverized Coal-Firing Boilers'),
                 tech('engine',name='Engines III'),tech('plane',name='Improved Small Airframe'),
                 tech('gun1',['gun2'],name='Infantry Equipment I'),tech('gun2',name='Infantry Equipment II')]
        g,s=definition(['Inter-war Tanks','Pulverized Coal Firing 1','Eng3','Engine 3',
                        'Improved Small Aircraft','Improved Infantry Equip UP'])
        r=preview(g,s,catalog)
        self.assertTrue(r['valid'],r['errors'])
        self.assertEqual([x['target'] for x in r['guide']['tracks']['research']],['tank','coal','engine','engine','plane','gun1'])
        self.assertEqual(r['guide']['tracks']['research'][-1]['relative_research']['chain'],['gun1','gun2'])

    def test_national_aircraft_model_nicknames_come_from_english_equipment_labels(self):
        definitions={'fighter1':{'name':'Fighter I','unlocks':[{'kind':'Equipaggiamento','id':'fighter_equipment_1'}]},
                     'fighter2':{'name':'Fighter II','unlocks':[{'kind':'Equipaggiamento','id':'fighter_equipment_2'}]},
                     'CAS2':{'name':'Close Air Support II','unlocks':[{'kind':'Equipaggiamento','id':'CAS_equipment_2'}]}}
        labels={'fighter1':'Fighter I','fighter2':'Fighter II','CAS2':'Close Air Support II',
                'ITA_fighter_equipment_1':'Macchi C.200 Saetta','ITA_fighter_equipment_2':'Macchi C.202 Folgore',
                'ITA_CAS_equipment_2':'Breda Ba.88 Lince'}
        catalog=catalog_entities({},definitions,labels,labels)
        g,s=definition(['Fighter Saetta','Fighter Folgore','CAS Lince'])
        r=preview(g,s,catalog)
        self.assertTrue(r['valid'],r['errors'])
        self.assertEqual([x['target'] for x in r['guide']['tracks']['research']],['fighter1','fighter2','CAS2'])
    def test_naval_shell_shorthands_select_incremental_shells_without_crossing_to_batteries_or_bombs(self):
        catalog=[tech('basic_light_shell',['battery'],name='Small-Caliber Semi-Armor Piercing Shell'),
                 tech('improved_light_shell',name='Small-Caliber Armor Piercing Shell'),
                 tech('basic_heavy_shell',['heavy_battery'],name='Armor-Piercing Capped Shell'),
                 tech('improved_heavy_shell',name='Super-Heavy Armor Piercing Shell'),
                 tech('battery'),tech('heavy_battery'),tech('armor_piercing_bombs',name='Armor Piercing Bombs')]
        guide,sources=definition(['Small Caliber…(navy)','Small-Caliber…',
                                  'Armor Piercing…(navy)','Armor Piercing…','Armor Piercing UP (navy)'])
        result=preview(guide,sources,catalog)
        self.assertTrue(result['valid'],result['errors'])
        chains=[s['relative_research']['chain'] for s in result['guide']['tracks']['research']]
        self.assertEqual(chains,[['basic_light_shell','improved_light_shell']]*2+[['basic_heavy_shell','improved_heavy_shell']]*3)
        self.assertTrue(preview(result['guide'],sources,catalog)['valid'])
        guide,sources=definition(['armor_piercing_bombs'])
        result=preview(guide,sources,catalog)
        self.assertNotIn('relative_research',result['guide']['tracks']['research'][0])
        # A missing member of a declared progression is not silently skipped.
        guide,sources=definition(['Small Caliber…(navy)'])
        self.assertFalse(preview(guide,sources,[r for r in catalog if r['id']!='improved_light_shell'])['valid'])
    def test_user_glossary_defines_tank_armor_and_infantry_guns_families(self):
        catalog=[tech('armor1',['armor2'],name='Basic Armor Protection'),
                 tech('armor2',name='Improved Armor Protection'),
                 tech('naval_armor',name='Basic Naval Armor'),
                 tech('guns1',['guns2'],name='Infantry Equipment I'),
                 tech('guns2',name='Infantry Equipment II'),tech('artillery',name='Artillery I')]
        guide,sources=definition(['Armor UP','Guns UP','Armor UP','Guns UP'])
        result=preview(guide,sources,catalog)
        self.assertTrue(result['valid'],result['errors'])
        self.assertEqual([s['relative_research']['chain'] for s in result['guide']['tracks']['research']],
                         [['armor1','armor2'],['guns1','guns2'],['armor1','armor2'],['guns1','guns2']])
    def test_plural_upgrades_names_a_progressive_family_without_changing_canonical_upgrade_names(self):
        guide,sources=definition(['Radio Upgrades'])
        result=preview(guide,sources,CATALOG)
        self.assertTrue(result['valid'],result['errors'])
        self.assertEqual(result['guide']['tracks']['research'][0]['relative_research']['chain'],
                         ['radio','improved_radio','advanced_radio'])
    def test_canonical_family_takes_precedence_over_country_equipment_acronym(self):
        from hoi4companion.research_upgrades import relative_chain
        rows=[{'kind':'technology','id':'aa1','name':'Anti-Air I','upgrade_successors':['aa2']},
              {'kind':'technology','id':'aa2','name':'Anti-Air II','upgrade_successors':[]},
              {'kind':'technology','id':'art3','name':'Medium SP Artillery III','aliases':['AA-3'],'upgrade_successors':[]}]
        self.assertEqual(relative_chain('AA',{r['id']:r for r in rows}),['aa1','aa2'])
    def test_detached_numbered_family_keeps_declared_level_progression(self):
        # Some game families (Excavation) have a construction prerequisite for
        # each level rather than paths between the levels themselves.
        catalog = [tech('ore_a', name='Extraction I'),
                   tech('ore_b', name='Extraction II'),
                   tech('ore_c', name='Extraction III')]
        before = copy.deepcopy(catalog)
        guide, sources = definition(['Extraction UP', 'Extraction UP'])
        report = preview(guide, sources, catalog)
        self.assertTrue(report['valid'], report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][0]['relative_research']['chain'],
                         ['ore_a', 'ore_b', 'ore_c'])
        self.assertEqual(catalog, before)  # Progression does not invent prerequisites.

    def test_detached_quality_family_keeps_explicit_named_levels(self):
        catalog = [tech('control_a', name='Basic Fire Control System'),
                   tech('control_b', name='Improved Fire Control System'),
                   tech('control_c', name='Advanced Fire Control System')]
        guide, sources = definition(['Fire Control System'])
        report = preview(guide, sources, catalog)
        self.assertTrue(report['valid'], report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][0]['relative_research']['chain'],
                         ['control_a', 'control_b', 'control_c'])

    def test_detached_progression_rejects_gaps_duplicate_levels_and_explicit_paths(self):
        for catalog in (
            [tech('a', name='Extraction I'), tech('b', name='Extraction III')],
            [tech('a', name='Extraction I'), tech('b', name='Extraction I')],
            [tech('a', ['outside'], name='Extraction I'), tech('b', name='Extraction II')],
            [tech('a', name='Improved Fire Control System'), tech('b', name='Advanced Fire Control System')],
        ):
            query = 'Fire Control System UP' if 'Fire' in catalog[0]['name'] else 'Extraction UP'
            guide, sources = definition([query])
            self.assertFalse(preview(guide, sources, catalog)['valid'], catalog)

    def test_unnumbered_unique_family_uses_next_useful_level(self):
        catalog=[tech('log1',['log2'],name='Logistics Company I'),tech('log2',name='Logistics Company II')]
        guide,sources=definition(['Logistics Company','Logistics UP'])
        report=preview(guide,sources,catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual([r['relative_research']['chain'] for r in report['guide']['tracks']['research']], [['log1','log2']]*2)

    def test_unrelated_unnumbered_families_are_not_automatically_selected(self):
        catalog=[tech('one',name='Medium Tank I'),tech('other',name='Medium Tank I')]
        guide,sources=definition(['Medium Tank'])
        self.assertFalse(preview(guide,sources,catalog)['valid'])
    def test_first_up_keeps_branch_and_does_not_bind_from_scenario(self):
        catalog = [tech('support_weapons', ['support_weapons2'], name='Support Weapons I'),
                   tech('support_weapons2', name='Support Weapons II'),
                   tech('engineers', ['engineers2'], name='Engineer Company I'),
                   tech('engineers2', name='Engineer Company II'),
                   tech('aa', ['aa2'], name='Anti-Air'), tech('aa2', name='Anti-Air Upgrade')]
        guide, sources = definition(['Support Weapons UP', 'Engineer Company UP', 'AA UP', 'Support Weapons'])
        context = {'country':'TST', 'date':'1936.1.1', 'researched':['engineers','aa'], 'unknown':[], 'basis':'country_history'}
        report = preview(guide, sources, catalog, research_context=context)
        self.assertTrue(report['valid'], report['errors'])
        rows=report['guide']['tracks']['research']
        self.assertEqual([s['relative_research']['chain'] for s in rows[:3]],
                         [['support_weapons','support_weapons2'],['engineers','engineers2'],['aa','aa2']])
        self.assertNotIn('completion', report['guide']['tracks']['research'][0])

    def test_first_up_does_not_use_start_and_does_not_choose_side_branch(self):
        catalog = [tech('engineers', ['engineers2'], name='Engineer Company I'),
                   tech('engineers2', name='Engineer Company II'),
                   tech('armored', name='Armored Engineer Company')]
        guide, sources = definition(['Engineer Company UP'])
        context = {'country':'TST','date':'1936.1.1','researched':['engineers'], 'unknown':[], 'basis':'country_history'}
        self.assertEqual(preview(guide,sources,catalog,research_context=context)['guide']['tracks']['research'][0]['relative_research']['chain'],['engineers','engineers2'])
        context['unknown'] = ['engineers']
        self.assertTrue(preview(guide,sources,catalog,research_context=context)['valid'])
        guide['applies_to']={'country':'OTH'}
        self.assertIn('nazione diversa',preview(guide,sources,catalog,research_context=context)['errors'][0])
    def test_repeated_upgrades_advance_the_same_family_not_the_latest_other_research(self):
        guide, sources = definition(['Basic Machine Tools','Radio','Tools UP','Radio UP','Tools UP (discount)','Radio UP','Tools UP'])
        before = copy.deepcopy(guide)
        report = preview(guide, sources, CATALOG)
        self.assertTrue(report['valid'], report['errors'])
        self.assertEqual([s['relative_research']['chain'][0] for s in report['guide']['tracks']['research'][2:]],
            ['basic_machine_tools','radio','basic_machine_tools','radio','basic_machine_tools'])
        self.assertEqual(guide, before)
        again = preview(report['guide'], sources, CATALOG)
        self.assertTrue(again['valid'])
        self.assertEqual(again['guide'], report['guide'])

    def test_up_keeps_branch_independent_of_declared_baseline(self):
        guide, sources = definition(['Radio UP','Radio UP'])
        unknown = preview(guide, sources, CATALOG)
        self.assertTrue(unknown['valid'])
        self.assertIn('upgrade_live', str(unknown['resolutions']))
        guide['research_baseline'] = ['radio']
        known = preview(guide, sources, CATALOG)
        self.assertTrue(known['valid'], known['errors'])
        self.assertEqual([s['relative_research']['chain'] for s in known['guide']['tracks']['research']],
                         [['radio','improved_radio','advanced_radio']]*2)

    def test_explicit_anchor_identifies_branch_without_fixing_a_level(self):
        guide, sources = definition(['Basic Machine Tools','Industry UP'])
        guide['tracks']['research'][1]['action']['params'] = {'upgrade_from':'r0'}
        report = preview(guide, sources, CATALOG)
        self.assertTrue(report['valid'], report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][1]['relative_research']['chain'],
                         ['basic_machine_tools','improved_machine_tools','advanced_machine_tools','assembly_line_production'])
        guide['tracks']['research'][1]['action']['params']['upgrade_from'] = 'missing'
        self.assertFalse(preview(guide, sources, CATALOG)['valid'])

    def test_end_branch_missing_target_and_failed_previous_upgrade_never_skip_or_guess(self):
        for catalog in ([tech('radio',['missing'])],):
            guide, sources = definition(['Radio','Radio UP','Radio UP'])
            report = preview(guide, sources, catalog)
            self.assertFalse(report['valid'])
            self.assertEqual([s['status'] for s in report['resolutions']], ['resolved','unresolved','unresolved'])
        catalog=[tech('radio',['one','two']),tech('one'),tech('two')]
        guide,sources=definition(['Radio','Radio UP'])
        report=preview(guide,sources,catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][1]['relative_research']['chain'],['radio'])

    def test_artillery_up_is_the_next_technology_not_just_the_next_equipment_unlock(self):
        catalog = [tech('gw_artillery',['interwar_artillery']), tech('interwar_artillery',['artillery1']),
                   tech('artillery1',['artillery2']), tech('artillery2',['artillery3']), tech('artillery3')]
        guide, sources = definition(['Artillery UP'] * 4, research_baseline=['gw_artillery'])
        report = preview(guide, sources, catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual([s['relative_research']['chain'] for s in report['guide']['tracks']['research']],
                         [['gw_artillery','interwar_artillery','artillery1','artillery2','artillery3']]*4)

    def test_a_manually_resolved_first_upgrade_keeps_its_authored_family_for_later_upgrades(self):
        guide, sources = definition(['Infantry Equip UP','Infantry Equip UP'])
        guide['tracks']['research'][0]['title'] = 'Infantry Equip UP'
        guide['tracks']['research'][0]['action']['entity']['id'] = 'weapons_b'
        catalog = [tech('weapons_b',['weapons_c']),tech('weapons_c')]
        report = preview(guide, sources, catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][1]['relative_research']['chain'],['weapons_b','weapons_c'])

    def test_shared_word_in_another_family_does_not_steal_the_upgrade_anchor(self):
        catalog = [tech('construction1',['construction2']),tech('construction2',['construction3']),
                   tech('construction3',['construction4']),tech('construction4'),tech('aircraft_construction')]
        guide, sources = definition(['Construction 1','Construction UP','Aircraft Construction','Construction UP'])
        report = preview(guide,sources,catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][-1]['relative_research']['chain'],['construction1','construction2','construction3','construction4'])
        guide, sources = definition(['Construction 1','Construction UP','construction3','Construction UP'])
        report = preview(guide,sources,catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][-1]['relative_research']['chain'],['construction1','construction2','construction3','construction4'])

    def test_catalog_disambiguates_mainline_from_side_branches_using_existing_gui_and_paths(self):
        def node(key, x, y, paths=()):
            return {'name':key,'paths':[{'target':target} for target in paths],
                    'folders':[{'id':'folder','layout':{'tree_id':'t','direction':'UP','anchor_x':x,'anchor_y':y}}]}
        catalog = catalog_entities({}, {'base':node('base',100,0,['next','side']),
            'next':node('next',100,100),'side':node('side',200,100)}, {})
        self.assertEqual(next(t for t in catalog if t['id']=='base')['upgrade_successors'], ['next'])
        # Geometry is optional; a genuine unresolved branch must remain ambiguous.
        bare = catalog_entities({}, {'a':{'name':'a','paths':[{'target':'b'},{'target':'c'}]},
                                     'b':{'name':'b'},'c':{'name':'c'}}, {})
        self.assertEqual(next(t for t in bare if t['id']=='a')['upgrade_successors'], ['b','c'])

    def test_runtime_without_run_binding_does_not_guess_upgrade_completion(self):
        guide, sources = definition(['Radio','Radio UP','Radio UP'])
        report = preview(guide, sources, CATALOG)
        self.assertTrue(report['valid'], report['errors'])
        def observed(value):return {'status':'observed','source':'fixture','value':value}
        state = {'schema_version':1,'usable':True,'facts':{'country':observed('ANY'),
                 'research.completed':observed(['radio','improved_radio']),
                 'research.active':observed([]),'research.slots':observed([{'technology_id':None}])}}
        result = evaluate(report['guide'],state)
        self.assertEqual([row['status'] for row in result['steps']], ['completed','unverifiable','unverifiable'])
        self.assertEqual(report['guide']['tracks']['research'][2]['relative_research']['chain'],['radio','improved_radio','advanced_radio'])


if __name__ == '__main__':unittest.main()
