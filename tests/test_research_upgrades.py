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
    def test_first_useful_level_with_or_without_up_uses_scenario_not_live_state(self):
        catalog = [tech('support_weapons', ['support_weapons2'], name='Support Weapons I'),
                   tech('support_weapons2', name='Support Weapons II'),
                   tech('engineers', ['engineers2'], name='Engineer Company I'),
                   tech('engineers2', name='Engineer Company II'),
                   tech('aa', ['aa2'], name='Anti-Air'), tech('aa2', name='Anti-Air Upgrade')]
        guide, sources = definition(['Support Weapons UP', 'Engineer Company UP', 'AA UP', 'Support Weapons'])
        context = {'country':'TST', 'date':'1936.1.1', 'researched':['engineers','aa'], 'unknown':[], 'basis':'country_history'}
        report = preview(guide, sources, catalog, research_context=context)
        self.assertTrue(report['valid'], report['errors'])
        self.assertEqual([s['target'] for s in report['guide']['tracks']['research']],
                         ['support_weapons','engineers2','aa2','support_weapons2'])
        self.assertNotIn('completion', report['guide']['tracks']['research'][0])

    def test_first_useful_preserves_unknown_start_and_does_not_choose_side_branch(self):
        catalog = [tech('engineers', ['engineers2'], name='Engineer Company I'),
                   tech('engineers2', name='Engineer Company II'),
                   tech('armored', name='Armored Engineer Company')]
        guide, sources = definition(['Engineer Company UP'])
        context = {'country':'TST','date':'1936.1.1','researched':['engineers'], 'unknown':[], 'basis':'country_history'}
        self.assertEqual(preview(guide,sources,catalog,research_context=context)['guide']['tracks']['research'][0]['target'],'engineers2')
        context['unknown'] = ['engineers']
        self.assertFalse(preview(guide,sources,catalog,research_context=context)['valid'])
        guide['applies_to']={'country':'OTH'}
        self.assertIn('nazione diversa',preview(guide,sources,catalog,research_context=context)['errors'][0])
    def test_repeated_upgrades_advance_the_same_family_not_the_latest_other_research(self):
        guide, sources = definition(['Basic Machine Tools','Radio','Tools UP','Radio UP','Tools UP (discount)','Radio UP','Tools UP'])
        before = copy.deepcopy(guide)
        report = preview(guide, sources, CATALOG)
        self.assertTrue(report['valid'], report['errors'])
        self.assertEqual([s['target'] for s in report['guide']['tracks']['research']],
            ['basic_machine_tools','radio','improved_machine_tools','improved_radio','advanced_machine_tools','advanced_radio','assembly_line_production'])
        self.assertEqual(guide, before)
        again = preview(report['guide'], sources, CATALOG)
        self.assertTrue(again['valid'])
        self.assertEqual(again['guide'], report['guide'])

    def test_first_upgrade_uses_a_declared_baseline_and_never_assumes_level_one(self):
        guide, sources = definition(['Radio UP','Radio UP'])
        unknown = preview(guide, sources, CATALOG)
        self.assertFalse(unknown['valid'])
        self.assertIn('upgrade_baseline_unknown', str(unknown['resolutions']))
        guide['research_baseline'] = ['radio']
        known = preview(guide, sources, CATALOG)
        self.assertTrue(known['valid'], known['errors'])
        self.assertEqual([s['target'] for s in known['guide']['tracks']['research']], ['improved_radio','advanced_radio'])

    def test_explicit_anchor_handles_abbreviations_and_is_import_only(self):
        guide, sources = definition(['Basic Machine Tools','Industry UP'])
        guide['tracks']['research'][1]['action']['params'] = {'upgrade_from':'r0'}
        report = preview(guide, sources, CATALOG)
        self.assertTrue(report['valid'], report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][1]['target'], 'improved_machine_tools')
        guide['tracks']['research'][1]['action']['params']['upgrade_from'] = 'missing'
        self.assertFalse(preview(guide, sources, CATALOG)['valid'])

    def test_end_branch_missing_target_and_failed_previous_upgrade_never_skip_or_guess(self):
        for catalog in ([tech('radio')], [tech('radio',['missing'])],
                        [tech('radio',['one','two']),tech('one'),tech('two')]):
            guide, sources = definition(['Radio','Radio UP','Radio UP'])
            report = preview(guide, sources, catalog)
            self.assertFalse(report['valid'])
            self.assertEqual([s['status'] for s in report['resolutions']], ['resolved','unresolved','unresolved'])

    def test_artillery_up_is_the_next_technology_not_just_the_next_equipment_unlock(self):
        catalog = [tech('gw_artillery',['interwar_artillery']), tech('interwar_artillery',['artillery1']),
                   tech('artillery1',['artillery2']), tech('artillery2',['artillery3']), tech('artillery3')]
        guide, sources = definition(['Artillery UP'] * 4, research_baseline=['gw_artillery'])
        report = preview(guide, sources, catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual([s['target'] for s in report['guide']['tracks']['research']],
                         ['interwar_artillery','artillery1','artillery2','artillery3'])

    def test_a_manually_resolved_first_upgrade_keeps_its_authored_family_for_later_upgrades(self):
        guide, sources = definition(['Infantry Equip UP','Infantry Equip UP'])
        guide['tracks']['research'][0]['title'] = 'Infantry Equip UP'
        guide['tracks']['research'][0]['action']['entity']['id'] = 'weapons_b'
        catalog = [tech('weapons_b',['weapons_c']),tech('weapons_c')]
        report = preview(guide, sources, catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][1]['target'],'weapons_c')

    def test_shared_word_in_another_family_does_not_steal_the_upgrade_anchor(self):
        catalog = [tech('construction1',['construction2']),tech('construction2',['construction3']),
                   tech('construction3',['construction4']),tech('construction4'),tech('aircraft_construction')]
        guide, sources = definition(['Construction 1','Construction UP','Aircraft Construction','Construction UP'])
        report = preview(guide,sources,catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][-1]['target'],'construction3')
        guide, sources = definition(['Construction 1','Construction UP','construction3','Construction UP'])
        report = preview(guide,sources,catalog)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['research'][-1]['target'],'construction4')

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

    def test_runtime_membership_does_not_rebind_the_imported_upgrade_to_current_play(self):
        guide, sources = definition(['Radio','Radio UP','Radio UP'])
        report = preview(guide, sources, CATALOG)
        self.assertTrue(report['valid'], report['errors'])
        def observed(value):return {'status':'observed','source':'fixture','value':value}
        state = {'schema_version':1,'usable':True,'facts':{'country':observed('ANY'),
                 'research.completed':observed(['radio','improved_radio']),
                 'research.active':observed([]),'research.slots':observed([{'technology_id':None}])}}
        result = evaluate(report['guide'],state)
        self.assertEqual([row['status'] for row in result['steps']], ['completed','completed','ready'])
        self.assertEqual(report['guide']['tracks']['research'][2]['target'],'advanced_radio')


if __name__ == '__main__':unittest.main()
