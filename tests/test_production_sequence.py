import copy,json,subprocess,unittest
from pathlib import Path
from hoi4companion.guide_importer import preview
from hoi4companion.guide_runtime import evaluate
from hoi4companion.resolution import CatalogIndex
from hoi4companion.informal_targets import production_command
from hoi4companion.screen_overlay import targets
from test_operation_import import document
from test_production_remaining import CATALOG,state,line


class ProductionSequenceTests(unittest.TestCase):
    def test_reached_phase_retires_old_targets_even_when_its_action_is_not_verifiable(self):
        from hoi4companion.production_milestones import token_for
        g=self.guide();anchor={'key':'advisor','action':{'kind':'manual','params':{'category':'politics'}},
            'completion':{'op':'fact','key':'politics.advisors','test':'contains','value':'expert'}}
        g['tracks']['adoptions']=[anchor]
        g['tracks']['operations'][1]['action']['params']['production_milestones']=[
            {'step':'advisor','token':token_for('advisor',anchor['completion']),'condition':anchor['completion']}]
        r=evaluate(g,self.live(['phase']));rows={x['key']:x for x in r['steps']}
        self.assertEqual(rows['order']['reason'],'production_superseded')
        self.assertIsNone(rows['order']['completion']['value'])
        self.assertEqual(rows['later']['reason'],'milestone_unknown')
        self.assertFalse(rows['later']['production_admitted'])
        self.assertEqual(targets(g,r,self.catalog(),{**self.live(['phase']),'sample':{'country':'TST'}}),[])
        # Unknown/future phase boundaries are not proof of progression.
        pending=evaluate(g,self.live());self.assertNotIn('superseded_by',next(x for x in pending['steps'] if x['key']=='order'))
        g['tracks']['operations'][1]['when']={'op':'fact','key':'unavailable','test':'eq','value':True}
        pending=evaluate(g,self.live(['phase']));self.assertNotIn('superseded_by',next(x for x in pending['steps'] if x['key']=='order'))

    def test_partial_replacement_of_a_blocked_phase_does_not_release_following_actions(self):
        from hoi4companion.production_milestones import token_for
        g=self.guide();anchor={'key':'advisor','action':{'kind':'manual'},'completion':{'op':'fact','key':'politics.advisors','test':'contains','value':'expert'}}
        g['tracks']['adoptions']=[anchor]
        g['tracks']['operations'][1]['action']['params']['production_milestones']=[
            {'step':'advisor','token':token_for('advisor',anchor['completion']),'condition':anchor['completion']}]
        g['tracks']['other']=[{'key':'after','action':{'kind':'manual'},'depends_on':['later']},
                              {'key':'after-start','action':{'kind':'manual'},'depends_on_started':['later']}]
        for known in (False,True):
            s=self.live(['phase'],['integrate'])
            if known:s['facts']['politics.advisors']={'status':'observed','source':'fixture','value':[]}
            rows={x['key']:x for x in evaluate(g,s)['steps']}
            self.assertEqual(rows['later']['reason'],'milestone_not_completed' if known else 'milestone_unknown')
            self.assertNotIn('superseded_by',rows['later']) # Explicit dependency target is protected.
            self.assertIsNone(rows['later']['completion']['value']);self.assertIsNone(rows['later']['selection']['value'])
            self.assertNotEqual(rows['after']['status'],'ready');self.assertNotEqual(rows['after-start']['status'],'ready')
            partial=copy.deepcopy(g);partial['tracks'].pop('other')
            blocked=next(x for x in evaluate(partial,s)['steps'] if x['key']=='later')
            self.assertIn('superseded_by',blocked)
            self.assertEqual(blocked['reason'],'milestone_not_completed' if known else 'milestone_unknown')
            self.assertIsNone(blocked['completion']['value']);self.assertIsNone(blocked['selection']['value'])


    def catalog(self):return CATALOG+[{'kind':'equipment','id':'tank','name':'Medium Tank','aliases':[],'equipment_ids':['tank']}]

    def guide(self):
        g,s=document({'category':'production','production_targets':[{'equipment_id':'sup','requested_factories':1},{'equipment_id':'inf','quantity_mode':'remaining','production_category':'military'}],
                      'production_revision':{'group':'military','order':0}})
        g['dependency_policy']='explicit'
        for key,order,items,when in [('later',10,[{'equipment_id':'sup','requested_factories':7},{'equipment_id':'inf','quantity_mode':'remaining','production_category':'military'}],
                                      {'op':'fact','key':'focus.completed','test':'contains','value':'phase'}),
                                     ('tanks',20,[{'equipment_id':'tank','quantity_mode':'remaining','production_category':'military'}],
                                      {'op':'fact','key':'focus.active','test':'contains','value':'integrate'})]:
            g['tracks']['operations'].append({'key':key,'action':{'kind':'manual','params':{'category':'production','production_targets':items,'production_revision':{'group':'military','order':order}}},'when':when,'evidence':copy.deepcopy(g['tracks']['operations'][0]['evidence'])})
        report=preview(g,s,self.catalog());self.assertTrue(report['valid'],report['errors']);return report['guide']

    def live(self,completed=(),active=()):
        s=state([line('1','sup',7,7),line('2','inf',10,10),line('3','tank',150,3)])
        for key,v in [('focus.completed',list(completed)),('focus.active',list(active)),('focus.tree','tree')]:s['facts'][key]={'status':'observed','value':v,'source':'fixture'}
        return s

    def test_phase_changes_replace_only_matching_goals_and_capacity_policy(self):
        g=self.guide();before=copy.deepcopy(g)
        initial=evaluate(g,self.live());self.assertEqual(initial['steps'][1]['status'],'waiting')
        middle=evaluate(g,self.live(['phase']));self.assertEqual(middle['steps'][0]['reason'],'production_superseded')
        self.assertIsNone(middle['steps'][0]['completion']['value'])
        self.assertEqual(middle['steps'][0]['original_completion']['value'],False)
        final=evaluate(g,self.live(['phase'],['integrate']))
        self.assertEqual(final['steps'][1]['reason'],'production_superseded')
        self.assertEqual(final['steps'][2]['status'],'completed')
        self.assertEqual(g,before)
        changed=self.live(['phase'],['integrate']);changed['facts']['production.lines']['value'][0].update(requested_factories=5,assigned_factories=5,active_factories=5)
        r=evaluate(g,changed);self.assertEqual(r['steps'][1]['reason'],'production_partially_superseded')
        hud=targets(g,r,self.catalog(),{**changed,'sample':{'country':'TST'}})
        self.assertEqual([p['identity'] for p in hud if p['goal_satisfied'] is not True],['sup'])
        self.assertEqual([p['identity'] for p in hud if p['goal_satisfied'] is True],['tank'])

    def test_unknown_condition_configured_counts_stale_and_wrong_campaign_do_not_advance(self):
        g=self.guide();s=self.live();s['facts'].pop('focus.completed')
        r=evaluate(g,s);self.assertEqual(r['steps'][1]['reason'],'condition_unknown')
        self.assertNotEqual(r['steps'][0]['reason'],'production_superseded')
        s['usable']=False;self.assertFalse(any(x.get('superseded_by') for x in evaluate(g,s)['steps']))
        s=self.live(['phase'],['integrate']);s['facts']['country']['value']='ALT'
        self.assertTrue(all(x['reason']=='guide_scope_mismatch' for x in evaluate(g,s)['steps']))

    def test_explicit_dependencies_and_independent_guides_are_preserved(self):
        g=self.guide();g['tracks']['operations'][1]['depends_on']=['order']
        r=evaluate(g,self.live(['phase']));self.assertNotIn('superseded_by',r['steps'][0])
        self.assertEqual(r['steps'][1]['status'],'waiting')
        for row in g['tracks']['operations']:row['action']['params'].pop('production_revision')
        self.assertFalse(any(x.get('superseded_by') for x in evaluate(g,self.live(['phase'],['integrate']))['steps']))

    def test_python_javascript_parity_in_all_revision_states(self):
        g=self.guide();states=[self.live(),self.live(['phase']),self.live(['phase'],['integrate'])]
        missing=self.live();missing['facts'].pop('focus.completed');states.append(missing)
        script='const r=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>process.stdout.write(JSON.stringify(JSON.parse(s).map(x=>r.evaluate(x.guide,x.state)))));'
        module=str(Path('src/hoi4companion/web/guide-runtime.js').resolve())
        rows=[{'guide':g,'state':s} for s in states]
        cross=copy.deepcopy(g);cross['tracks']['other']=[{'key':'cross','action':{'kind':'manual'},'depends_on_started':['later']}]
        rows.append({'guide':cross,'state':missing})
        from hoi4companion.production_milestones import token_for
        partial=self.guide();condition={'op':'fact','key':'politics.advisors','test':'contains','value':'expert'}
        partial['tracks']['adoptions']=[{'key':'advisor','action':{'kind':'manual'},'completion':condition}]
        partial['tracks']['operations'][1]['action']['params']['production_milestones']=[{'step':'advisor','token':token_for('advisor',condition),'condition':condition}]
        rows.append({'guide':partial,'state':self.live(['phase'],['integrate'])})
        result=subprocess.run(['node','-e',script,module],input=json.dumps(rows),text=True,capture_output=True,check=True)
        self.assertEqual(json.loads(result.stdout),[evaluate(x['guide'],x['state']) for x in rows])

    def test_unprefixed_allocations_ranges_and_unresolved_shared_quantities(self):
        index=CatalogIndex(self.catalog())
        result=production_command('Train, Truck:1, Sup:7, Inf: ALL',index,'TST')
        self.assertEqual([x.get('requested_factories') for x in result['params']['production_targets']],[1,7,None])
        self.assertIn('Train',result['remainder'])
        self.assertEqual(production_command('Inf:0->10',index,'TST')['params']['production_targets'][0]['requested_factories'],10)
        self.assertIsNone(production_command('Defend the border: 10',index,'TST'))
        self.assertIsNone(production_command('Inf:10->0',index,'TST'))

    def test_word_section_anchors_and_ellipsis_use_declared_objectives(self):
        from test_informal_operations import InformalOperationsTests
        catalog=self.catalog()+[{'kind':'focus','id':'f','name':'Integrate Regional Industries','aliases':[],'scope':'tree'}]
        helper=InformalOperationsTests()
        def add_focus(g):
            g['tracks']['focus']=[{'key':'focus','action':{'kind':'select_focus','entity':{'kind':'focus','id':'f','scope':'tree'}},'evidence':copy.deepcopy(g['tracks']['operations'][0]['evidence'])}]
        report,sources=helper.report(['On Integrate…: Sup:7, Inf:ALL'],catalog,edit=add_focus)
        row=next(r for r in report['guide']['tracks']['operations'] if r['action']['params'].get('category')=='production')
        self.assertEqual(row['when']['args'][0]['value'],'f')
        self.assertIn('production_revision',row['action']['params'])
        self.assertEqual(preview(report['guide'],sources,catalog)['guide'],report['guide'])

    def test_later_word_section_requires_an_observed_milestone_even_if_counts_match(self):
        from hoi4companion.import_sources import acquire_source
        first='MIL: Sup:1, Inf:ALL';later='Sup:7, Inf:ALL'
        raw='[Table row]\n[Column 1]\n'+first+'\n[Column 2]\n[Column 3]\nFirst Milestone\n[Table row]\n[Column 1]\n'+later+'\n[Column 2]\n[Column 3]\nLater Milestone'
        source=acquire_source(text=raw);source['type']='file'
        g={'id':'sections','title':'Sectioned production','schema_version':1,'dependency_policy':'explicit','applies_to':{'country':'TST'},'sources':[source],'tracks':{'operations':[],'focus':[]}}
        def evidence(quote):return [{'source_id':source['id'],'basis':'explicit','quote':quote}]
        for i,text in enumerate((first,later)):
            g['tracks']['operations'].append({'key':'p'+str(i),'tracking':'reminder','action':{'kind':'manual','params':{'instructions':[text],'category':'checkpoints'}},'evidence':evidence(text)})
        catalog=self.catalog()
        for i,title in enumerate(('First Milestone','Later Milestone')):
            catalog.append({'kind':'focus','id':'f'+str(i),'name':title,'aliases':[],'scope':'tree'})
            g['tracks']['focus'].append({'key':'f'+str(i),'action':{'kind':'select_focus','entity':{'kind':'focus','id':'f'+str(i),'scope':'tree'}},'evidence':evidence(title)})
        report=preview(g,[source],catalog);self.assertTrue(report['valid'],report['errors']);g=report['guide']
        before=evaluate(g,self.live());by={x['key']:x for x in before['steps']}
        self.assertEqual(by['p1']['status'],'waiting');self.assertNotIn('superseded_by',by['p0'])
        entered=evaluate(g,self.live(['f0']));by={x['key']:x for x in entered['steps']}
        self.assertEqual(by['p0']['reason'],'production_superseded')
        self.assertEqual(preview(g,[source],catalog)['guide'],g)

    def test_malformed_revision_contract_is_rejected(self):
        from hoi4companion.guide import normalise_guide
        g=self.guide()
        for revision in ({'group':'military','order':True},{'group':'','order':0},{'group':'x','order':-1},{'group':'x','order':1,'extra':True}):
            bad=copy.deepcopy(g);bad['tracks']['operations'][0]['action']['params']['production_revision']=revision
            with self.assertRaises(ValueError):normalise_guide(bad)

    def test_configured_future_allocation_cannot_release_cross_track_dependencies(self):
        g=self.guide()
        g['tracks']['other']=[{'key':'after','action':{'kind':'manual'},'depends_on':['later']}]
        g['tracks']['other'].append({'key':'after-start','action':{'kind':'manual'},'depends_on_started':['later']})
        s=self.live();s['facts'].pop('focus.completed')
        r={x['key']:x for x in evaluate(g,s)['steps']}
        self.assertEqual(r['later']['reason'],'condition_unknown')
        self.assertIsNone(r['later']['selection']['value'])
        self.assertEqual(r['after']['reason'],'dependency_unknown')
        self.assertEqual(r['after-start']['reason'],'dependency_unknown')
