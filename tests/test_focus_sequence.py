"""Source-order identity proofs; no game access and no strategy generation."""
from copy import deepcopy
import json
from pathlib import Path
import unittest

from hoi4companion.focus_sequence import reconstruct
from hoi4companion.guide import normalise_guide
from hoi4companion.guide_importer import preview
from hoi4companion.import_sources import acquire_source
from hoi4companion.guide_runtime import evaluate


def node(key, groups=(), *, name=None, mutex=(), bypass=False):
    return {'kind':'focus','id':key,'name':name or key,'scope':'tree','countries':['TST'],
            'aliases':[],'prerequisites':[list(g) for g in groups],
            'mutually_exclusive':list(mutex),'bypass_possible':bypass,'sequence_effects_safe':True}


def document(queries, *, coverage='complete', initial=()):
    src=acquire_source(text='\n'.join(queries))
    guide={'id':'sequence','title':'Source sequence','applies_to':{'country':'TST','tree_id':'tree'},
           'sources':[{'id':src['id'],'type':'manual'}],
           'tracks':{'focus':[{'key':f'slot-{i+1}',
              'action':{'kind':'select_focus','entity':{'kind':'focus','id':q}},
              'evidence':[{'source_id':src['id'],'basis':'explicit','quote':q}]}
              for i,q in enumerate(queries)]}}
    if coverage is not None:guide['focus_sequence']={'schema_version':1,'coverage':coverage,'initial_focuses':list(initial)}
    return guide,[src]


class FocusSequenceTests(unittest.TestCase):
    def test_interleaved_branches_recover_the_unique_unreadable_slot(self):
        entries=[node('F1')]+[node(f'F{i}',[[f'F{i-1}']]) for i in range(2,10)]
        entries += [node('F10',[['F6']])]+[node(f'F{i}',[[f'F{i-1}']]) for i in range(11,16)]
        queries=[*[f'F{i}' for i in range(1,7)],'F10','F11','F12','???','F8']
        g,s=document(queries);before=deepcopy(g)
        r=preview(g,s,entries)
        self.assertTrue(r['valid'],r['errors']);self.assertEqual(g,before)
        row=r['guide']['tracks']['focus'][9]
        self.assertEqual(row['target'],'F7');self.assertEqual(row['key'],'slot-10')
        self.assertEqual(row['focus_resolution']['position'],10)
        self.assertEqual(row['focus_resolution']['query'],'???')
        self.assertEqual(row['focus_resolution']['classification'],'reconstructed')
        self.assertEqual(r['resolutions'][9]['matching_basis'],'focus_sequence_constraints')

    def test_ten_consecutive_unreadable_focuses_are_reconstructed_bidirectionally(self):
        entries=[node('F1')]+[node(f'F{i}',[[f'F{i-1}']]) for i in range(2,13)]
        g,s=document(['F1',*['???']*10,'F12'])
        r=preview(g,s,entries)
        self.assertTrue(r['valid'],r['errors'])
        self.assertEqual([x['target'] for x in r['guide']['tracks']['focus']],[f'F{i}' for i in range(1,13)])
        self.assertEqual(r['focus_sequence']['reconstructed_count'],10)

    def test_independent_and_parents_do_not_prove_their_order(self):
        entries=[node('A'),node('B',[['A']]),node('C',[['A']]),node('Z',[['B'],['C']])]
        g,s=document(['A','???','???','Z']);r=preview(g,s,entries)
        self.assertFalse(r['valid']);self.assertEqual(r['focus_sequence']['reconstructed_count'],0)
        self.assertEqual([x['target'] for x in r['guide']['tracks']['focus']][1:3],['???','???'])

    def test_free_choice_may_satisfy_a_prerequisite_without_forcing_another_hole(self):
        entries=[node('A'),node('B',[['A']]),node('P',[['A']]),node('Z',[['B']])]
        g,s=document(['A','???','Z'])
        g['tracks']['focus'].insert(1,{'key':'free','action':{'kind':'free'},
            'evidence':deepcopy(g['tracks']['focus'][0]['evidence'])})
        r=preview(g,s,entries)
        self.assertFalse(r['valid']);self.assertEqual(r['focus_sequence']['reconstructed_count'],0)
        self.assertEqual(r['guide']['tracks']['focus'][2]['focus_resolution']['position'],3)
        self.assertEqual(r['guide']['tracks']['focus'][1]['action']['kind'],'free')

    def test_free_choice_is_never_rewritten_even_if_the_only_valid_witness_is_known(self):
        entries=[node('A'),node('B',[['A']]),node('Z',[['B']])]
        g,s=document(['A','Z'])
        g['tracks']['focus'].insert(1,{'key':'free','action':{'kind':'free'},
            'evidence':deepcopy(g['tracks']['focus'][0]['evidence'])})
        r=preview(g,s,entries)
        self.assertTrue(r['valid'],r['errors'])
        self.assertEqual(r['guide']['tracks']['focus'][1]['action'],{'kind':'free'})
        self.assertEqual(r['guide']['tracks']['focus'][2]['focus_resolution']['position'],3)

    def test_or_alternatives_are_not_arbitrarily_selected(self):
        entries=[node('A'),node('B',[['A']]),node('C',[['A']]),node('Z',[['B','C']])]
        g,s=document(['A','???','Z']);r=preview(g,s,entries)
        self.assertFalse(r['valid']);self.assertEqual(r['focus_sequence']['status'],'validated')
        self.assertEqual(r['guide']['tracks']['focus'][1]['target'],'???')

    def test_later_anchor_and_mutex_constrain_an_earlier_or(self):
        entries=[node('A'),node('B',[['A']],mutex=['C']),node('C',[['A']]),
                 node('Z',[['B','C']]),node('W',[['B']])]
        g,s=document(['A','???','Z','W']);r=preview(g,s,entries)
        self.assertTrue(r['valid'],r['errors']);self.assertEqual(r['guide']['tracks']['focus'][1]['target'],'B')

    def test_partial_is_default_and_never_uses_slot_count_as_proof(self):
        entries=[node('A'),node('B',[['A']]),node('Z',[['B']])]
        for coverage in (None,'partial'):
            g,s=document(['A','???','Z'],coverage=coverage);r=preview(g,s,entries)
            self.assertFalse(r['valid']);self.assertEqual(r['focus_sequence']['status'],'partial')
        g,s=document(['Z','A'],coverage='partial')
        self.assertTrue(preview(g,s,entries)['valid'])

    def test_possible_and_unknown_bypasses_weaken_the_proof(self):
        for bypass in (True,None):
            entries=[node('A'),node('B',[['A']],bypass=bypass),node('P',[['A']]),node('Z',[['B']])]
            g,s=document(['A','???','Z']);r=preview(g,s,entries)
            self.assertFalse(r['valid']);self.assertEqual(r['focus_sequence']['reconstructed_count'],0)

    def test_route_does_not_report_a_false_conflict_from_a_bypassable_ancestor(self):
        entries=[node('A',mutex=['B']),node('B',bypass=True),node('Z',[['B']])]
        g,s=document(['A','Z']);r=preview(g,s,entries)
        self.assertTrue(r['valid'],r['errors'])
        self.assertNotIn('B',[x['id'] for x in r['focus_route']['required_focuses']])

    def test_joint_and_empty_bypass_metadata_are_taken_from_real_parser_nodes(self):
        from hoi4companion.clausewitz import parse
        from hoi4companion.focus import _build_focus
        root=parse('id=A bypass={} completion_reward={}')
        self.assertFalse(_build_focus(root,{},'fixture').bypass_possible)
        self.assertTrue(_build_focus(root,{},'fixture',jointly_completed=True).external_completion_possible)
        root=parse('id=A completion_reward={ if={limit={always=yes} load_focus_tree={tree=B}} }')
        self.assertFalse(_build_focus(root,{},'fixture').sequence_effects_safe)

    def test_propagation_proofs_match_exhaustive_small_graph_solutions(self):
        from itertools import permutations
        import random
        rng=random.Random(1729)
        for case in range(70):
            ids=list('ABCDE');entries=[]
            for i,key in enumerate(ids):
                earlier=ids[:i];groups=[]
                if earlier and rng.random()<.8:groups.append(rng.sample(earlier,rng.randint(1,len(earlier))))
                if earlier and rng.random()<.3:groups.append([rng.choice(earlier)])
                entries.append(node(key,groups,bypass=rng.random()<.15))
            if rng.random()<.5:
                a,b=rng.sample(ids,2);entries[ids.index(a)]['mutually_exclusive']=[b]
                entries[ids.index(b)]['mutually_exclusive']=[a]
            initial=['A'] if rng.random()<.2 else []
            queries=[rng.choice(ids) if rng.random()<.45 else '???' for _ in range(4)]
            g,_=document(queries,initial=initial);g=normalise_guide(g)
            named={r['key']:{'candidate_domain':[x for x in entries if x['id']==q]}
                   for r,q in zip(g['tracks']['focus'],queries)}
            by_id={n['id']:n for n in entries};bypassed={n['id'] for n in entries if n['bypass_possible']}
            solutions=[]
            for seq in permutations(ids,4):
                if any(q!='???' and q!=k for q,k in zip(queries,seq)) or set(seq)&set(initial):continue
                selected=set(seq)|set(initial)
                if any(set(by_id[k]['mutually_exclusive'])&selected for k in selected):continue
                done=set(initial);valid=True
                for k in seq:
                    if k not in bypassed and any(not set(group)&(done|bypassed) for group in by_id[k]['prerequisites']):valid=False;break
                    done.add(k)
                if valid:solutions.append(seq)
            forced,report,errors=reconstruct(g,entries,named)
            with self.subTest(case=case):
                if not solutions:self.assertEqual(report['status'],'conflict')
                else:
                    expected={f'slot-{i+1}':solutions[0][i] for i in range(4) if len({s[i] for s in solutions})==1}
                    self.assertEqual({k:v['id'] for k,v in forced.items()},expected)
                    self.assertEqual(errors,[])

    def test_initial_completed_focus_cannot_be_forced_into_a_source_hole(self):
        entries=[node('A'),node('B',[['A']]),node('P',[['A']]),node('Q',[['A']]),node('Z',[['B']])]
        g,s=document(['???','Z'],initial=['A','B']);r=preview(g,s,entries)
        self.assertFalse(r['valid']);self.assertEqual(r['focus_sequence']['reconstructed_count'],0)

    def test_weak_typo_binding_is_corrected_by_global_constraints(self):
        entries=[node('A'),node('P',[['A']],name='Develop Industrial Plan'),
                 node('R',[['A']],name='Develop Industrial Plans'),node('Z',[['R']])]
        g,s=document(['A','Develop Industral Plan','Z']);r=preview(g,s,entries)
        self.assertTrue(r['valid'],r['errors']);self.assertEqual(r['guide']['tracks']['focus'][1]['target'],'R')
        self.assertEqual(r['resolutions'][1]['classification'],'reconstructed')

    def test_exact_source_name_and_explicit_id_are_not_silently_replaced(self):
        entries=[node('A'),node('P',[['A']],name='Parallel Industry'),node('R',[['A']]),node('Z',[['R']])]
        for query in ('P','Parallel Industry','native_missing_id'):
            g,s=document(['A',query,'Z']);r=preview(g,s,entries)
            self.assertFalse(r['valid']);self.assertEqual(r['focus_sequence']['status'],'conflict')
            self.assertNotEqual(r['guide']['tracks']['focus'][1]['target'],'R')

    def test_conflicting_exact_name_gets_a_probable_repair_without_overwriting_it(self):
        entries=[node('A'),node('P',[['A']],name='Parallel Industry'),node('R',[['A']]),node('Z',[['R']])]
        g,s=document(['A','Parallel Industry','Z']);r=preview(g,s,entries)
        self.assertFalse(r['valid']);self.assertEqual(r['guide']['tracks']['focus'][1]['target'],'P')
        repair=r['focus_sequence']['probable_repairs'][0]
        self.assertEqual((repair['position'],repair['entity']['id'],repair['classification']),(2,'R','probable'))
        self.assertEqual(r['focus_sequence']['conflicts'][0]['required_any_of'],['R'])

    def test_manual_binding_is_locked_and_source_query_is_preserved(self):
        entries=[node('A'),node('P',[['A']]),node('R',[['A']]),node('Z',[['R']])]
        g,s=document(['A','???','Z']);row=g['tracks']['focus'][1]
        row['action']['entity']['id']='P'
        row['focus_resolution']={'schema_version':1,'position':2,'query':'???','classification':'direct',
                                 'entity':deepcopy(row['action']['entity']),'locked':True}
        r=preview(g,s,entries)
        self.assertFalse(r['valid']);self.assertEqual(r['guide']['tracks']['focus'][1]['focus_resolution']['query'],'???')

    def test_revalidation_does_not_turn_an_inferred_id_into_independent_evidence(self):
        entries=[node('A'),node('B',[['A']]),node('Z',[['B']])]
        g,s=document(['A','???','Z']);r=preview(g,s,entries);self.assertTrue(r['valid'],r['errors'])
        again=preview(r['guide'],s,entries);self.assertTrue(again['valid'],again['errors'])
        self.assertEqual(r['guide'],again['guide'])
        entries[1]['bypass_possible']=True;entries.append(node('P',[['A']]))
        changed=preview(r['guide'],s,entries)
        self.assertFalse(changed['valid']);self.assertEqual(changed['guide']['tracks']['focus'][1]['action']['entity']['id'],'???')
        self.assertNotEqual(changed['resolutions'][1]['status'],'resolved')

    def test_global_order_conflicts_do_not_validate(self):
        entries=[node('A'),node('B',[['A']])]
        g,s=document(['B','A']);r=preview(g,s,entries)
        self.assertFalse(r['valid']);self.assertEqual(r['focus_sequence']['status'],'conflict')

    def test_only_explicitly_repeatable_focuses_can_occupy_two_source_slots(self):
        for repeatable,valid in ((True,True),(False,False)):
            entries=[dict(node('A'),repeatable=repeatable)]
            g,s=document(['A','A']);r=preview(g,s,entries)
            self.assertEqual(r['valid'],valid,r['errors'])

    def test_missing_cyclic_or_reset_relations_remain_unverifiable(self):
        for entries in ([node('A'),node('Z',[['missing']])],
                        [node('A',[['Z']]),node('Z',[['A']])],
                        [dict(node('A'),sequence_effects_safe=False),node('Z',[['A']])]):
            g,s=document(['A','???','Z']);r=preview(g,s,entries)
            self.assertEqual(r['focus_sequence']['status'],'incomplete')
            self.assertEqual(r['focus_sequence']['reconstructed_count'],0)

    def test_limit_never_publishes_a_partial_proof(self):
        entries=[node('A'),node('B',[['A']]),node('Z',[['B']])]
        g,_=document(['A','???','Z']);g=normalise_guide(g)
        named={r['key']:{'candidate_domain':[x for x in entries if x['id']==r['action']['entity']['id']]}
               for r in g['tracks']['focus']}
        forced,report,errors=reconstruct(g,entries,named,work_limit=1)
        self.assertEqual(forced,{});self.assertEqual(report['status'],'limit');self.assertEqual(errors,[])

    def test_conditions_and_conditional_dependencies_do_not_assert_a_completed_step(self):
        entries=[node('A'),node('B',[['A']]),node('Z',[['B']])]
        for dependent in (False,True):
            g,s=document(['A','???','Z'])
            if dependent:
                g['tracks']['operations']=[{'key':'gate','action':{'kind':'manual'},'when':{'op':'check','key':'optional'}}]
                g['tracks']['focus'][1]['depends_on']=['gate']
            else:g['tracks']['focus'][1]['when']={'op':'check','key':'optional'}
            r=preview(g,s,entries);self.assertEqual(r['focus_sequence']['status'],'validated')
            self.assertEqual(r['focus_sequence']['conditional_slot_count'],1)
            # The unconditional successor forces the missing identity even
            # though its live when condition is deliberately not evaluated.
            self.assertEqual(r['guide']['tracks']['focus'][1]['target'],'B')

    def test_optional_slot_cannot_force_a_prerequisite_before_the_wrong_anchor(self):
        entries=[node('A'),node('B',[['A']]),node('P',[['A']]),node('Z',[['B']])]
        g,s=document(['A','B','???','Z']);g['tracks']['focus'][1]['when']={'op':'check','key':'choose'}
        r=preview(g,s,entries)
        self.assertFalse(r['valid']);self.assertEqual(r['focus_sequence']['reconstructed_count'],0)

    def test_joint_focus_can_be_completed_outside_the_players_source_slots(self):
        entries=[node('A'),dict(node('B',[['A']]),external_completion_possible=True),
                 node('P',[['A']]),node('Z',[['B']])]
        g,s=document(['A','???','Z']);r=preview(g,s,entries)
        self.assertFalse(r['valid']);self.assertEqual(r['focus_sequence']['external_completion_count'],1)
        self.assertEqual(r['focus_sequence']['reconstructed_count'],0)

    def test_conditional_fallback_preserves_the_authored_action_not_the_proof_witness(self):
        entries=[node('A'),node('B',[['A']]),node('C',[['A']]),node('Z',[['B']])]
        g,s=document(['A','B (Bypass) / C (if late with the campaign)','Z','C'])
        r=preview(g,s,entries)
        self.assertTrue(r['valid'],r['errors'])
        row=r['guide']['tracks']['focus'][1]
        self.assertEqual(row['target'],'C')
        self.assertEqual(row['action']['params']['conditional_focus']['bypass_id'],'B')
        self.assertIn('when',row)

    def test_identity_proof_is_not_live_completion(self):
        entries=[node('A'),node('B',[['A']]),node('Z',[['B']])]
        g,s=document(['A','???','Z']);r=preview(g,s,entries)
        state={'schema_version':1,'usable':True,'facts':{
            'country':{'status':'observed','source':'fixture','value':'TST'},
            'focus.completed':{'status':'observed','source':'fixture','value':[]}}}
        result=evaluate(r['guide'],state)
        self.assertTrue(all(row['status']!='completed' for row in result['steps']))

    def test_preserved_source_positions_cannot_be_reordered(self):
        entries=[node('A'),node('B',[['A']])]
        g,s=document(['A','B']);r=preview(g,s,entries)
        r['guide']['tracks']['focus'].reverse()
        with self.assertRaisesRegex(ValueError,'position'):normalise_guide(r['guide'])

    def test_contract_fixture_matches_python_validation(self):
        fixture=json.loads((Path(__file__).parent/'fixtures'/'focus-sequence-contract.json').read_text())
        for item in fixture:
            with self.subTest(item['name']):
                try:normalise_guide(item['guide']);valid=True
                except ValueError:valid=False
                self.assertEqual(valid,item['valid'])
