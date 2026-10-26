import copy
import json
import subprocess
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from hoi4companion.political_import import political_catalog, bind_political
from hoi4companion.guide_importer import Importer
from hoi4companion.import_sources import acquire_source
from hoi4companion.clausewitz import parse
from hoi4companion.event_country import country_condition, event_condition


class PoliticalBindingTests(unittest.TestCase):
    def test_commander_is_distinct_from_a_government_appointment_and_not_auto_completed(self):
        with TemporaryDirectory() as directory:
            root=Path(directory);path=root/'common/characters/test.txt';path.parent.mkdir(parents=True)
            path.write_text('characters={TST_person={name=person_name corps_commander={traits={infantry_leader}} advisor={slot=high_command idea_token=expert}}}')
            rows,_=political_catalog(root,[],{}, {'person_name':'Test Person'})
        commanders=[r for r in rows if r['kind']=='commander']
        self.assertEqual(commanders[0]['slot'],'corps_commander')
        step={'key':'unit','action':{'kind':'manual','params':{'person_name':'Test Person','role':'corps_commander'}}}
        self.assertEqual(bind_political(step,rows,'TST'),'commander_assignment_unobserved')
        self.assertEqual(step['action']['params']['commander_id'],'TST_person')
        self.assertNotIn('completion',step)
        step={'key':'staff','action':{'kind':'manual','params':{'person_name':'Test Person','role':'high_command'}}}
        self.assertEqual(bind_political(step,rows,'TST'),'politics_live_advisor')
        self.assertEqual(step['completion']['value'],'expert')

    def test_role_mismatch_is_never_discarded_to_resolve_an_advisor(self):
        rows=[{'kind':'advisor','id':'p','name':'Person','character_id':'c','slot':'high_command'}]
        for role in ['political_advisor','unknown_role']:
            step={'key':'s','action':{'kind':'manual','params':{'advisor_id':'p','role':role}}}
            self.assertIn('unresolved',bind_political(step,rows,None))
            self.assertNotIn('completion',step)

    def test_political_country_constraints_cover_roles_and_decision_categories_without_availability_guesses(self):
        files={
            'common/ideas/laws.txt':'''ideas={economy={law=yes global={allowed={always=no}} regional={allowed={original_tag=FRA}}}
                country={GER_misleading={allowed={tag=FRA}} shared={allowed={always=no}}}}''',
            'common/characters/people.txt':'''characters={person={advisor={slot=political_advisor idea_token=foreign_role allowed={original_tag=FRA}}
                advisor={slot=high_command idea_token=shared_role available={has_war=yes}}}}''',
            'common/decisions/categories/test.txt':'''national={allowed={original_tag=FRA} visible={has_war=yes}}''',
            'common/decisions/test.txt':'''national={choice={available={has_political_power>50}}}
                generic={GER_false={allowed={tag=FRA}} common={target_trigger={tag=FRA}}}''',
        }
        with TemporaryDirectory() as d:
            root=Path(d)
            for name,text in files.items():
                path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text)
            rows,warnings=political_catalog(root,[],{},{});self.assertEqual(warnings,[])
        by_id={r['id']:r for r in rows}
        self.assertEqual(by_id['regional']['country_condition'],{'op':'original_tag','value':'FRA'})
        self.assertEqual(by_id['foreign_role']['country_condition'],{'op':'original_tag','value':'FRA'})
        self.assertEqual(by_id['shared']['country_condition'],{'op':'unknown'})
        self.assertEqual(by_id['global']['country_condition'],{'op':'unknown'})
        self.assertEqual(by_id['GER_misleading']['country_condition'],{'op':'tag','value':'FRA'})
        self.assertEqual(by_id['choice']['country_condition'],{'op':'all','args':[{'op':'original_tag','value':'FRA'},{'op':'unknown'}]})
        self.assertEqual(by_id['common']['country_condition'],{'op':'unknown'})
        self.assertNotIn('allowed',by_id);self.assertNotIn('visible',by_id)
        keys=['regional','foreign_role','shared','global','GER_misleading','choice','common']
        script="const fs=require('fs'),m=require('./src/hoi4companion/web/importer-model.js');console.log(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>m.countryCompatibility(x,'GER','GER'))));"
        output=subprocess.run(['node','-e',script],input=json.dumps([by_id[k]['country_condition'] for k in keys]),capture_output=True,text=True,check=True)
        self.assertEqual(json.loads(output.stdout),[False,False,None,None,False,False,None])

    def test_python_country_projection_and_javascript_filter_share_three_valued_semantics(self):
        cases=[('tag=GER',True),('tag=FRA',False),('tag=GER has_war=yes',None),
               ('tag=FRA has_war=yes',False),('OR={tag=FRA has_war=yes}',None),
               ('OR={tag=GER has_war=yes}',True),('NOT={tag=FRA}',True),
               ('NOT={has_war=yes}',None),('NOT={tag=FRA tag=ENG}',None),
               ('original_tag=FRA',None),('FROM={tag=FRA}',None),('tag=ROOT',None),
               ('tag != GER',False),('tag ?= FRA',None),('always=no',False),
               ('AND={OR={tag=GER tag=ENG} NOT={tag=FRA}}',True)]
        data=[country_condition(parse(text)) for text,_ in cases]
        script="const fs=require('fs'),m=require('./src/hoi4companion/web/importer-model.js');console.log(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>m.countryCompatibility(x,'GER'))));"
        output=subprocess.run(['node','-e',script],input=json.dumps(data),capture_output=True,text=True,check=True)
        self.assertEqual(json.loads(output.stdout),[expected for _,expected in cases])
        self.assertEqual(event_condition('state_event',parse('trigger={tag=FRA}')),{'op':'unknown'})

    def test_duplicate_event_options_merge_country_alternatives_without_losing_labels(self):
        with TemporaryDirectory() as d:
            root=Path(d);(root/'events').mkdir()
            (root/'events/a.txt').write_text('country_event={id=e trigger={tag=GER} option={name=answer trigger={tag=GER}} option={name=answer trigger={tag=FRA}}}')
            rows,_=political_catalog(root,[],{},{} )
        option=next(r for r in rows if r['kind']=='event_option')
        self.assertEqual(option['country_condition'],{'op':'any','args':[{'op':'tag','value':'GER'},{'op':'tag','value':'FRA'}]})

    def test_event_catalog_retains_recipient_conditions_without_confusing_foreign_scopes(self):
        with TemporaryDirectory() as d:
            root=Path(d);(root/'events').mkdir()
            (root/'events/test.txt').write_text('''
country_event={id=foreign_title trigger={tag=GER has_war=yes FRA={tag=FRA}}
 option={name=yes trigger={NOT={tag=FRA}}} option={name=no}}
country_event={id=GER_not_german trigger={OR={tag=FRA tag=ENG}} option={name=one}}
country_event={id=complex trigger={OR={tag=FRA has_country_flag=variable}} option={name=two}}
news_event={id=news trigger={tag=FRA} major=yes option={name=three}}
country_event={id=major trigger={tag=FRA} major=yes show_major={tag=GER} option={name=four}}
country_event={id=not_equal trigger={tag != FRA} option={name=five}}
''')
            rows,warnings=political_catalog(root,[],{},{});self.assertEqual(warnings,[])
        by_id={r['id']:r for r in rows}
        self.assertEqual(by_id['GER_not_german']['country_condition'],{'op':'any','args':[{'op':'tag','value':'FRA'},{'op':'tag','value':'ENG'}]})
        self.assertEqual(by_id['foreign_title']['country_condition'],{'op':'all','args':[{'op':'tag','value':'GER'},{'op':'unknown'}]})
        self.assertEqual(by_id['yes']['country_condition'],{'op':'not','arg':{'op':'tag','value':'FRA'}})
        self.assertEqual(by_id['news']['country_condition'],{'op':'true'})
        self.assertEqual(by_id['major']['country_condition'],{'op':'any','args':[{'op':'tag','value':'FRA'},{'op':'tag','value':'GER'}]})
        self.assertEqual(by_id['not_equal']['country_condition'],{'op':'not','arg':{'op':'tag','value':'FRA'}})

    def test_ai_packet_keeps_relevant_politics_without_all_world_events(self):
        catalog=[{'kind':'event','id':'e.'+str(n),'name':'Unique Unmentioned Event '+str(n)} for n in range(4000)]
        catalog += [{'kind':'law','id':'war_economy','name':'War Economy'},
                    {'kind':'event','id':'example.1','name':'A New Sovereign'},
                    {'kind':'event_option','id':'example.1.a','event_id':'example.1','name':'Yes, accept!'}]
        packet=Importer(lambda:catalog).packet([acquire_source(text='Choose War Economy and await A New Sovereign.')])
        self.assertEqual({x['id'] for x in packet['catalog']},{'war_economy','example.1','example.1.a'})
    def catalog(self, root):
        files = {
            'common/ideas/laws.txt': 'ideas={economy={law=yes war_economy={cost=150}} designer={electronic_concern={traits={electronics}}}}',
            'common/characters/TST.txt': 'characters={TST_person={name=TST_person advisor={slot=political_advisor idea_token=TST_advisor traits={noble}}}}',
            'events/example.txt': 'country_event={id=example.1 title=example.1.t option={name=example.1.a}}',
            'common/decisions/example.txt': 'political_actions={war_bonds={cost=50}}',
        }
        for filename, text in files.items():
            path=root/filename; path.parent.mkdir(parents=True,exist_ok=True); path.write_text(text)
        english={'war_economy':'War Economy','TST_person':'Test Person','noble':'Noble Bureaucrat',
                 'example.1.t':'A New Sovereign','example.1.a':'Accept the Candidate','war_bonds':'War Bonds'}
        return political_catalog(root,[],{'war_economy':'Economia di guerra'},english)

    def test_static_catalog_preserves_english_ids_and_installed_aliases(self):
        with TemporaryDirectory() as folder:
            rows,warnings=self.catalog(Path(folder))
        self.assertEqual(warnings,[])
        law=next(r for r in rows if r['id']=='war_economy')
        self.assertEqual(law['name'],'War Economy'); self.assertIn('Economia di guerra',law['aliases'])
        advisor=next(r for r in rows if r['kind']=='advisor')
        self.assertEqual(advisor['id'],'TST_advisor'); self.assertEqual(advisor['character_id'],'TST_person')
        self.assertIn('Noble Bureaucrat',advisor['aliases'])
        option=next(r for r in rows if r['kind']=='event_option')
        self.assertEqual(option['event_id'],'example.1')

    def test_law_and_trait_advisor_bind_to_different_qualified_memberships(self):
        with TemporaryDirectory() as folder: rows,_=self.catalog(Path(folder))
        for params,expected in [({'law_name':'Economia di guerra'},'politics.ideas'),
                                ({'trait_name':'Noble Bureaucrat','role':'political_advisor'},'politics.advisors')]:
            step={'key':'s','action':{'kind':'manual','params':params}}
            reason=bind_political(step,rows,'TST')
            self.assertIn('live',reason); self.assertEqual(step['completion']['key'],expected)
        # A character being recruited is not proof of the requested appointment.
        self.assertEqual(step['completion']['value'],'TST_advisor')

    def test_same_trait_in_multiple_countries_requires_country_scope(self):
        with TemporaryDirectory() as folder: rows,_=self.catalog(Path(folder))
        rows.append({**next(r for r in rows if r['kind']=='advisor'),'id':'OTH_advisor','character_id':'OTH_person','countries':['OTH']})
        source={'key':'s','action':{'kind':'manual','params':{'trait_name':'Noble Bureaucrat'}}}
        step=copy.deepcopy(source); self.assertEqual(bind_political(step,rows,None),'politics_identity_ambiguous')
        self.assertNotIn('completion',step)
        step=copy.deepcopy(source); self.assertEqual(bind_political(step,rows,'TST'),'politics_live_advisor')
        self.assertEqual(step['action']['params']['advisor_id'],'TST_advisor')

    def test_event_option_presence_only_marks_started_never_chosen(self):
        with TemporaryDirectory() as folder: rows,_=self.catalog(Path(folder))
        step={'key':'s','action':{'kind':'manual','params':{'event_name':'A New Sovereign','event_option':'Accept the Candidate'}}}
        self.assertEqual(bind_political(step,rows,'TST'),'event_visible_option')
        self.assertEqual(step['action']['params']['event_id'],'example.1')
        self.assertEqual(step['started_when']['value'],'example.1:example.1.a')
        self.assertNotIn('completion',step)

    def test_unknown_explicit_ids_do_not_fall_back_to_names(self):
        with TemporaryDirectory() as folder: rows,_=self.catalog(Path(folder))
        for params in [{'law_id':'wrong','law_name':'War Economy'},
                       {'event_id':'wrong','event_option':'Accept the Candidate'}]:
            step={'key':'s','action':{'kind':'manual','params':params}}
            self.assertIn('unresolved',bind_political(step,rows,'TST')); self.assertNotIn('completion',step)

    def test_combined_project_is_not_completed_just_by_naming_its_advisor(self):
        with TemporaryDirectory() as folder:rows,_=self.catalog(Path(folder))
        step={'key':'s','action':{'kind':'manual','params':{'category':'projects','project_name':'Lead Intelligence','person_name':'Test Person'}}}
        self.assertIsNone(bind_political(step,rows,'TST'));self.assertNotIn('completion',step)

    def test_legacy_concern_named_by_identity_uses_selected_ideas(self):
        rows=[{'kind':'idea','id':'TEST_concern','name':'Example Industries','group':'industrial_concern'}]
        step={'key':'s','action':{'kind':'manual','params':{'category':'characters','person_name':'Example Industries'}}}
        self.assertEqual(bind_political(step,rows,'TST'),'politics_live_idea')
        self.assertEqual(step['completion']['key'],'politics.ideas')

    def test_decision_identity_is_not_availability_and_manual_criterion_preserved(self):
        with TemporaryDirectory() as folder: rows,_=self.catalog(Path(folder))
        manual={'op':'check','key':'explicit-source-outcome'}
        step={'key':'s','action':{'kind':'manual','params':{'decision_name':'War Bonds'}},'completion':manual.copy()}
        self.assertEqual(bind_political(step,rows,'TST'),'decision_identity_resolved')
        self.assertNotIn('started_when',step)
        self.assertEqual(step['completion'],manual)

    def test_revalidation_replaces_only_generated_political_conditions(self):
        rows=[{'kind':'law','id':key,'group':'economy','name':key} for key in ('one','two')]
        step={'key':'s','action':{'kind':'manual','params':{'law_name':'one'}}}
        bind_political(step,rows,'TST'); step['action']['params']['law_id']='two'
        bind_political(step,rows,'TST'); self.assertEqual(step['completion']['value'],'two')
        step['completion']={'op':'check','key':'custom'};bind_political(step,rows,'TST')
        self.assertEqual(step['completion'],{'op':'check','key':'custom'})
