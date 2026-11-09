from copy import deepcopy
from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion.guide import normalise_guide
from test_focus_sequence import node, document

class NativeFocusSequenceTests(TestCase):
    def test_whole_route_resolution_matches_independent_oracle(self):
        from hoi4companion._focus_resolution_reference import resolve_focus_routes,infer_focus_scope
        entries=[node('A'),node('B',[['A']],name='B Industry',mutex=['P']),node('P',[['A']],name='P Industry'),node('Z',[['B']])]
        for queries in (['A','???','Z'],['A','B Industy','Z'],['A','P Industry','Z'],['A','???','???','Z']):
            guide,_=document(queries);guide=normalise_guide(guide)
            actual=_request(0,'focus_resolve_routes',guide=guide,entries=entries)
            self.assertEqual((actual['resolutions'],actual['route'],actual['errors']),resolve_focus_routes(guide,entries))
            del guide['applies_to']['tree_id']
            self.assertEqual(_request(0,'focus_infer_scope',guide=guide,entries=entries),infer_focus_scope(guide,entries))

    def test_bounded_graph_search_matches_independent_python_oracle(self):
        import random
        from hoi4companion._focus_sequence_reference import reconstruct,source_guide
        rng=random.Random(68429)
        for case in range(100):
            entries=[]
            for i,key in enumerate('ABCDE'):
                earlier=list('ABCDE'[:i]);groups=[]
                if earlier and rng.random()<.8:groups.append(rng.sample(earlier,rng.randint(1,len(earlier))))
                if earlier and rng.random()<.4:groups.append([rng.choice(earlier)])
                entries.append(node(key,groups,bypass=rng.choice([False,False,True,None])))
            if rng.random()<.5:
                a,b=rng.sample(range(5),2);entries[a]['mutually_exclusive']=[entries[b]['id']]
            guide,_=document([rng.choice('ABCDE') if rng.random()<.5 else '???' for _ in range(4)],initial=['A'] if rng.random()<.2 else [])
            source=source_guide(normalise_guide(guide))
            named={r['key']:{'candidate_domain':[e for e in entries if e['id']==r['action']['entity']['id']]} for r in source['tracks']['focus']}
            for limit in (0,10,2000000):
                expected=reconstruct(source,entries,named,work_limit=limit)
                actual=_request(0,'focus_reconstruct',guide=source,entries=entries,named=named,work_limit=limit)
                with self.subTest(case=case,limit=limit):self.assertEqual((actual['forced'],actual['report'],actual['errors']),expected)

    def test_compatibility_adapter_preserves_retained_row_references(self):
        from hoi4companion.conditional_focus import compile_conditional_focuses
        from hoi4companion.resolution import catalog_index
        from test_conditional_focus import ConditionalFocusTests
        guide,_,entries=ConditionalFocusTests().definition();row=guide['tracks']['focus'][0]
        compile_conditional_focuses(guide,catalog_index(entries))
        self.assertIs(row,guide['tracks']['focus'][0])
        self.assertEqual(row['target'],'exercises')

    def test_native_counterexample_proof_and_source_slots(self):
        entries=[node('A'),node('B',[['A']]),node('Z',[['B']])]
        guide,_=document(['A','???','Z']);guide=normalise_guide(guide)
        source=_request(0,'focus_source_guide',guide=guide)
        self.assertEqual([r['_focus_position'] for r in source['tracks']['focus']],[1,2,3])
        named={r['key']:{'candidate_domain':[e for e in entries if e['id']==r['action']['entity']['id']]} for r in source['tracks']['focus']}
        result=_request(0,'focus_reconstruct',guide=source,entries=entries,named=named,work_limit=2000000)
        self.assertEqual(result['forced']['slot-2']['id'],'B')
        self.assertEqual(result['report']['status'],'validated')
        self.assertEqual(result['errors'],[])
        limited=_request(0,'focus_reconstruct',guide=source,entries=entries,named=named,work_limit=0)
        self.assertEqual(limited['report']['status'],'limit')
        self.assertEqual(limited['forced'],{})

    def test_conditional_focus_preserves_declared_conditions_and_unknowns(self):
        from test_conditional_focus import ConditionalFocusTests
        guide,_,entries=ConditionalFocusTests().definition()
        before=deepcopy(guide)
        result=_request(0,'focus_compile_conditionals',guide=guide,entries=entries)
        row=result['guide']['tracks']['focus'][0]
        self.assertEqual(row['action']['entity']['id'],'exercises')
        self.assertEqual(row['action']['params']['conditional_focus']['condition_basis'],'source_text_unverifiable')
        self.assertTrue(row['when']['key'].startswith('guide.conditions.'))
        self.assertEqual(guide,before)
