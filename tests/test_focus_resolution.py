import copy
import unittest

from hoi4companion.guide_importer import preview
from hoi4companion.import_sources import acquire_source


def focus(key,name,prerequisites=(),mutex=()):
    return {'kind':'focus','id':key,'name':name,'aliases':[], 'scope':'test',
            'prerequisites':list(prerequisites),'mutually_exclusive':list(mutex)}


def definition(names):
    src=acquire_source(text='\n'.join(names))
    return {'schema_version':1,'id':'focus-names','title':'Guide','sources':[{'id':src['id'],'type':'manual'}],
            'tracks':{'focus':[{'key':f'f{i}','action':{'kind':'select_focus','entity':{'kind':'focus','id':name}},
                'evidence':[{'source_id':src['id'],'basis':'explicit','quote':name}]} for i,name in enumerate(names)]}},[src]


class FocusResolutionTests(unittest.TestCase):
    def test_single_word_typo_is_reported_and_exact_identity_stays_first(self):
        g,s=definition(['Fulfill the 5th of November Act']); before=copy.deepcopy(g)
        entities=[focus('act','Fulfil the 5th of November Act')]
        report=preview(g,s,entities)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['focus'][0]['target'],'act')
        self.assertEqual(report['resolutions'][0]['matching_basis'],'minor_typo')
        self.assertTrue(any('refuso' in w for w in report['warnings']))
        self.assertEqual(g,before)
        entities.append(focus('literal','Fulfill the 5th of November Act'))
        self.assertEqual(preview(g,s,entities)['guide']['tracks']['focus'][0]['target'],'literal')

    def test_ties_short_words_levels_and_unrelated_names_never_autocorrect(self):
        for query,names in [('Construction IV',['Construction III']),('Radio',['Ratio']),
                            ('Research modern aircraft',['Research medium aircraft']),
                            ('Build Factory',['Build Factories']),
                            ('Fulfill the Act',['Fulfil the Act','Fullill the Act'])]:
            g,s=definition([query]); entities=[focus(str(i),n) for i,n in enumerate(names)]
            self.assertFalse(preview(g,s,entities)['valid'],query)

    def catalog(self):
        return [focus('shared','Shared Root'),focus('path_a','Path A',[['shared']],['path_b']),
                focus('path_b','Path B',[['shared']],['path_a']),
                focus('claim_a','Demand Land',[['path_a']]),focus('claim_b','Demand Land',[['path_b']])]

    def test_same_names_use_declared_preceding_prerequisite_not_id_suffix(self):
        g,s=definition(['Path B','Demand Land'])
        report=preview(g,s,self.catalog())
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['focus'][1]['target'],'claim_b')
        self.assertEqual(report['resolutions'][1]['matching_basis'],'guide_prerequisites')
        self.assertEqual(report['resolutions'][1]['supporting_focus_ids'],['path_b'])
        self.assertNotIn('completion',report['guide']['tracks']['focus'][1])

    def test_unrelated_conditional_or_conflicting_route_does_not_choose(self):
        for names,conditional in [(['Shared Root','Demand Land'],False),
                                  (['Path A','Path B','Demand Land'],False),
                                  (['Path B','Demand Land'],True)]:
            g,s=definition(names)
            if conditional:g['tracks']['focus'][0]['when']={'op':'check','key':'optional'}
            self.assertFalse(preview(g,s,self.catalog())['valid'],names)

    def test_and_of_or_prerequisites_require_all_groups_and_allow_valid_alternative(self):
        entities=self.catalog()+[focus('extra','Extra')]
        entities[-3]['prerequisites']=[['path_a','path_b'],['extra']]
        g,s=definition(['Path B','Demand Land'])
        # An omitted extra prerequisite is not proof that a candidate is excluded.
        self.assertFalse(preview(g,s,entities)['valid'])
        entities[-2]['prerequisites']=[['path_b'],['extra']]
        self.assertFalse(preview(g,s,entities)['valid'])


class GlobalFocusRouteTests(unittest.TestCase):
    def catalog(self):
        return [focus('root','Shared Root'),focus('a','Path A',[['root']],['b']),
                focus('b','Path B',[['root']],['a']),focus('middle_a','Middle A',[['a']]),
                focus('middle_b','Middle B',[['b']]),
                focus('claim_a','Demand Land',[['middle_a']]),
                focus('claim_b','Demand Land',[['middle_b']])]

    def test_transitive_route_and_later_steps_exclude_the_other_branch(self):
        for names in (['Path B','Demand Land'],['Demand Land','Middle B']):
            g,s=definition(names); before=copy.deepcopy(g)
            report=preview(g,s,self.catalog())
            self.assertTrue(report['valid'],report['errors'])
            row=next(r for r in report['resolutions'] if r.get('entity',{}).get('id')=='claim_b')
            self.assertEqual(row['excluded_candidates'][0]['entity']['id'],'claim_a')
            self.assertEqual(row['excluded_candidates'][0]['reason'],'route_mutual_exclusion')
            self.assertIn('b',row['supporting_focus_ids'])
            self.assertEqual(g,before)
            self.assertNotIn('completion',report['guide']['tracks']['focus'][names.index('Demand Land')])
            again=preview(report['guide'],s,self.catalog())
            self.assertTrue(again['valid'])
            self.assertEqual(again['guide'],report['guide'])
            self.assertEqual(again['resolutions'][names.index('Demand Land')]['excluded_candidates'],row['excluded_candidates'])

    def test_fixpoint_propagates_newly_resolved_names_into_other_ambiguities(self):
        entities=[focus('block','Blocker',mutex=['x_a']),focus('a','Path A',mutex=['b']),
                  focus('b','Path B',mutex=['a']),focus('x_a','First Choice'),
                  focus('x_b','First Choice',[['b']]),focus('y_a','Second Choice',[['a']]),
                  focus('y_b','Second Choice',[['b']])]
        g,s=definition(['Second Choice','First Choice','Blocker'])
        report=preview(g,s,entities)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual([r['target'] for r in report['guide']['tracks']['focus']],['y_b','x_b','block'])

    def test_exact_focuses_with_incompatible_mandatory_ancestors_block_import(self):
        g,s=definition(['Middle A','Middle B'])
        report=preview(g,s,self.catalog())
        self.assertFalse(report['valid'])
        self.assertEqual(report['focus_route']['status'],'conflict')
        self.assertTrue(any('percorso' in e.lower() for e in report['errors']))

    def test_shared_or_ancestor_is_not_treated_as_a_selected_branch(self):
        entities=self.catalog()+[focus('either','Either Path',[['a','b']])]
        g,s=definition(['Either Path','Demand Land'])
        report=preview(g,s,entities)
        self.assertFalse(report['valid'])
        self.assertNotIn('a',[r['id'] for r in report['focus_route']['required_focuses']])
        self.assertNotIn('b',[r['id'] for r in report['focus_route']['required_focuses']])

    def test_and_or_pruning_never_excludes_a_viable_or_alternative(self):
        entities=self.catalog()+[focus('or_claim','Shared Claim',[['a','b']]),
                                focus('and_claim','Shared Claim',[['a'],['b']])]
        g,s=definition(['Path B','Shared Claim'])
        report=preview(g,s,entities)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['guide']['tracks']['focus'][1]['target'],'or_claim')

    def test_conditional_phase_and_conditional_dependencies_cannot_force_a_route(self):
        for mode in ['phase','dependency','skip']:
            g,s=definition(['Middle B','Demand Land'])
            if mode=='phase':
                g['phases']=[{'id':'optional','title':'Optional','when':{'op':'check','key':'branch'}}]
                g['tracks']['focus'][0]['phase']='optional'
            elif mode=='dependency':
                g['tracks']['operations']=[{'key':'gate','action':{'kind':'manual'},'when':{'op':'check','key':'branch'},
                    'evidence':g['tracks']['focus'][0]['evidence']}]
                g['tracks']['focus'][0]['depends_on']=['gate']
            else:g['tracks']['focus'][0]['skip_when']={'op':'check','key':'branch'}
            self.assertFalse(preview(g,s,self.catalog())['valid'],mode)

    def test_absent_or_cyclic_catalog_paths_remain_unknown(self):
        for broken in ('missing','cycle'):
            entities=self.catalog()
            entities[-1]['prerequisites']=[['unknown' if broken=='missing' else 'claim_b']]
            g,s=definition(['Path B','Demand Land'])
            report=preview(g,s,entities)
            self.assertFalse(report['valid'],broken)
            resolution=report['resolutions'][1]
            self.assertEqual(resolution['status'],'unresolved_route')
