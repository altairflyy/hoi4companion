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
    def test_one_changed_word_resolves_unique_focus_and_records_the_original_label(self):
        for query,label in [('Reinstate The Herrenbuilding','Reinstate the Herrenhaus'),
                            ('Je Suis La Deluge','Je Suis Le Deluge'),
                            ('Build Factory','Build Factories')]:
            g,s=definition([query])
            g['applies_to']={'tree_id':'test'};before=copy.deepcopy(g)
            report=preview(g,s,[focus('native',label)])
            self.assertTrue(report['valid'],report['errors'])
            row=report['resolutions'][0]
            self.assertEqual(row['entity']['id'],'native')
            self.assertEqual(row['matching_basis'],'focus_one_word_variant')
            self.assertEqual(row['association_evidence']['matched_label'],label)
            self.assertEqual(len(row['association_evidence']['changed_words']),1)
            self.assertEqual(g,before)
            self.assertNotIn('completion',report['guide']['tracks']['focus'][0])
            self.assertTrue(preview(report['guide'],s,[focus('native',label)])['valid'])

    def test_word_variants_keep_ambiguity_and_do_not_override_exact_or_minor_typos(self):
        g,s=definition(['Reinstate The Herrenbuilding'])
        g['applies_to']={'tree_id':'test'}
        entries=[focus('one','Reinstate the Herrenhaus'),focus('two','Reinstate the House')]
        self.assertFalse(preview(g,s,entries)['valid'])
        entries.append(focus('exact','Reinstate The Herrenbuilding'))
        self.assertEqual(preview(g,s,entries)['resolutions'][0]['entity']['id'],'exact')
        g,s=definition(['Fulfill the Act'])
        report=preview(g,s,[focus('minor','Fulfil the Act'),focus('other','Fulfil an Act')])
        self.assertEqual(report['resolutions'][0]['matching_basis'],'minor_typo')

    def test_word_variants_preserve_scope_country_levels_directions_and_semantic_qualifiers(self):
        for query,label in [('Modern Aircraft Research','Medium Aircraft Research'),
                            ('Industry Level III','Industry Level II'),
                            ('The Eastern Claims','The Western Claims'),
                            ('Do Not Surrender','Do Now Surrender'),
                            ('Radio','Ratio'),('Path A','Path B'),
                            ('First Missing Words','Second Different Words'),
                            ('Reinstate/Keep The Council','Reinstate The Council'),
                            ('unknown_native_id','Unknown Native Focus')]:
            g,s=definition([query])
            g['applies_to']={'tree_id':'test'}
            self.assertFalse(preview(g,s,[focus('native',label)])['valid'],query)
        g,s=definition(['Reinstate The Herrenbuilding']);g['applies_to']={'country':'AUS','tree_id':'test'}
        report=preview(g,s,[{**focus('wrong_country','Reinstate the Herrenhaus'),'countries':['GER']},
                            {**focus('wrong_scope','Reinstate the Herrenhaus'),'scope':'other'}])
        self.assertFalse(report['valid'])

    def test_word_variants_use_the_declared_route_without_forcing_parallel_choices(self):
        entries=[focus('a','Political Path Alpha',mutex=['b']),focus('b','Political Path Beta',mutex=['a']),
                 focus('one','Reinstate the Herrenhaus',[['a']]),
                 focus('two','Reinstate the House',[['b']])]
        g,s=definition(['Political Path Alpha','Reinstate The Herrenbuilding'])
        g['applies_to']={'tree_id':'test'}
        report=preview(g,s,entries)
        self.assertTrue(report['valid'],report['errors'])
        self.assertEqual(report['resolutions'][1]['entity']['id'],'one')
        self.assertEqual(report['resolutions'][1]['matching_basis'],'focus_one_word_variant')
        g['tracks']['focus'][0]['when']={'op':'check','key':'optional'}
        self.assertFalse(preview(g,s,entries)['valid'])

    def test_word_variants_require_known_national_context_and_a_complete_catalog_path(self):
        g,s=definition(['Work With The Integralists'])
        self.assertFalse(preview(g,s,[{**focus('USA_work','Work With The Bund'),'countries':['USA']}])['valid'])
        g,s=definition(['Reinstate The Herrenbuilding']);g['applies_to']={'tree_id':'test'}
        for parents in ([['missing']], [['native']]):
            self.assertFalse(preview(g,s,[focus('native','Reinstate the Herrenhaus',parents)])['valid'])

    def test_word_variant_never_steals_a_named_decision_or_another_entity(self):
        g,s=definition(['Authorize A Greater Italy']);g['applies_to']={'country':'ITA','tree_id':'test'}
        entries=[{**focus('towards','Towards a Greater Italy'),'countries':['ITA']},
                 {'kind':'decision','id':'proclaim','name':'Authorize Greater Italy','countries':['ITA']}]
        self.assertFalse(preview(g,s,entries)['valid'])
        g,s=definition(['Towards a Greater Italy']);g['applies_to']={'country':'ITA','tree_id':'test'}
        self.assertEqual(preview(g,s,entries)['resolutions'][0]['entity']['id'],'towards')

    def test_omitted_or_extra_word_and_bounded_letter_errors_keep_identity(self):
        for query,label in [('The Rur Dam','Build the Rur Dam'),
                            ('The Imperial Rule Of Assistance Association','The Imperial Rule Assistance Association'),
                            ('Panzer Troop Schools','Panzer Troops School'),
                            ('Fallschirmjaer','Fallschirmjäger'),
                            ('Subdoing The Baltics States','Subduing the Baltic States'),
                            ('Proclaim A Greater Italy','Towards a Greater Italy')]:
            g,s=definition([query]);g['applies_to']={'tree_id':'test'}
            r=preview(g,s,[focus('native',label)])
            self.assertTrue(r['valid'],(query,r['errors']))
            self.assertEqual(r['resolutions'][0]['entity']['id'],'native')
        for query,label in [('Expand The Eastern Border','Expand The Border'),
                            ('Do Not Surrender','Do Surrender'),
                            ('Industrial Effort II','Industrial Effort')]:
            g,s=definition([query]);g['applies_to']={'tree_id':'test'}
            self.assertFalse(preview(g,s,[focus('native',label)])['valid'],query)

    def test_declared_government_disambiguates_branch_variants_without_claiming_live_availability(self):
        entries=[{**focus('normal','Army Glory'),'excluded_governments':['fascism','neutrality']},
                 {**focus('alt','Army Glory'),'excluded_governments':['communism','democratic']}]
        for government,expected in [('Communist','normal'),('Non-Aligned','alt')]:
            g,s=definition(['Army Glory']);g['title']='Country ('+government+') Strategy'
            result=preview(g,s,entries)
            self.assertTrue(result['valid'],result['errors'])
            self.assertEqual(result['guide']['tracks']['focus'][0]['target'],expected)
            self.assertEqual(result['resolutions'][0]['matching_basis'],'guide_government_branch')
            self.assertNotIn('completion',result['guide']['tracks']['focus'][0])
        g,s=definition(['Army Glory'])
        self.assertFalse(preview(g,s,entries)['valid'])
    def recovery_catalog(self):
        return [focus('one','First Focus'),focus('two','Second Focus',[['one']]),
                focus('three','Third Focus',[['two']]),
                focus('four','Reinstate the Herrenhaus',[['three']]),
                focus('five','The Danubian Federation',[['four']]),
                focus('parallel','Parallel Industry',[['one']])]

    def test_misspelled_required_ancestor_is_recovered_from_later_focus_not_ui_adjacency(self):
        g,s=definition(['First Focus','Third Focus','Reinstate Tha Herrenhouse',
                        'Parallel Industry','The Danubian Federation'])
        before=copy.deepcopy(g)
        report=preview(g,s,self.recovery_catalog())
        self.assertTrue(report['valid'],report['errors'])
        row=report['resolutions'][2]
        self.assertEqual(row['entity']['id'],'four')
        self.assertEqual(row['matching_basis'],'guide_required_ancestor')
        self.assertIn('five',row['supporting_focus_ids'])
        self.assertEqual(report['guide']['tracks']['focus'][3]['target'],'parallel')
        self.assertNotIn('completion',report['guide']['tracks']['focus'][2])
        self.assertEqual(g,before)

    def test_required_ancestor_does_not_overwrite_a_plausible_parallel_name_or_an_arbitrary_unknown(self):
        for query,extra in [('Reinstate Tha Herrenhouse',[focus('parallel_four','Reinstate the Herrenhauser',[['one']])]),
                            ('A',[])]:
            g,s=definition(['Third Focus',query,'The Danubian Federation'])
            report=preview(g,s,self.recovery_catalog()+extra)
            self.assertFalse(report['valid'],query)
            self.assertNotEqual(report['resolutions'][1]['status'],'resolved')

    def test_or_prerequisite_does_not_identify_the_missing_focus(self):
        entries=self.recovery_catalog()
        entries[-2]['prerequisites']=[['four','parallel']]
        g,s=definition(['Third Focus','Reinstate Tha Herrenhouse','The Danubian Federation'])
        self.assertFalse(preview(g,s,entries)['valid'])

    def test_conditional_future_focus_or_broken_graph_cannot_recover_a_typo(self):
        for mode in ('conditional','missing','cycle'):
            entries=self.recovery_catalog()
            g,s=definition(['Third Focus','Reinstate Tha Herrenhouse','The Danubian Federation'])
            if mode=='conditional':g['tracks']['focus'][2]['when']={'op':'check','key':'optional'}
            elif mode=='missing':entries[-2]['prerequisites']=[['four'],['absent']]
            else:entries[-2]['prerequisites']=[['four'],['five']]
            self.assertFalse(preview(g,s,entries)['valid'],mode)

    def test_recovered_catalog_identity_does_not_complete_unobserved_live_focus(self):
        from hoi4companion.guide_runtime import evaluate
        g,s=definition(['Third Focus','Reinstate Tha Herrenhouse','The Danubian Federation'])
        report=preview(g,s,self.recovery_catalog())
        self.assertTrue(report['valid'],report['errors'])
        fact=lambda value:{'status':'observed','source':'fixture','value':value}
        state={'schema_version':1,'usable':True,'facts':{'focus.tree':fact('test'),'focus.active':fact([])}}
        result=evaluate(report['guide'],state)
        self.assertIsNone(result['steps'][1]['completion']['value'])

    def test_named_level_or_direction_is_not_replaced_by_a_required_other_level_or_direction(self):
        for query,label in [('Industrial Research III','Industrial Research II'),
                            ('The Eastward Expansion','The Westward Expansion')]:
            entries=[focus('before','Before Focus'),focus('required',label,[['before']]),
                     focus('after','After Focus',[['required']])]
            g,s=definition(['Before Focus',query,'After Focus'])
            self.assertFalse(preview(g,s,entries)['valid'],query)

    def test_started_dependency_on_conditional_step_cannot_force_the_future_route(self):
        g,s=definition(['Third Focus','Reinstate Tha Herrenhouse','The Danubian Federation'])
        g['tracks']['operations']=[{'key':'gate','action':{'kind':'manual'},
            'when':{'op':'check','key':'optional'},'evidence':g['tracks']['focus'][0]['evidence']}]
        g['tracks']['focus'][2]['depends_on_started']=['gate']
        self.assertFalse(preview(g,s,self.recovery_catalog())['valid'])

    def test_exact_parallel_focus_is_never_replaced_by_a_required_missing_ancestor(self):
        entries=self.recovery_catalog()+[focus('parallel_four','Reinstate The Herrenhouse',[['one']])]
        g,s=definition(['Third Focus','Reinstate The Herrenhouse','The Danubian Federation'])
        result=preview(g,s,entries)
        self.assertTrue(result['valid'],result['errors'])
        self.assertEqual(result['guide']['tracks']['focus'][1]['target'],'parallel_four')
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
