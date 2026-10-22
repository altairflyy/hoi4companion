import copy
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from hoi4companion.political_import import political_catalog, bind_political


class PoliticalBindingTests(unittest.TestCase):
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

    def test_decision_presence_is_not_completion_and_manual_criterion_preserved(self):
        with TemporaryDirectory() as folder: rows,_=self.catalog(Path(folder))
        manual={'op':'check','key':'explicit-source-outcome'}
        step={'key':'s','action':{'kind':'manual','params':{'decision_name':'War Bonds'}},'completion':manual.copy()}
        self.assertEqual(bind_political(step,rows,'TST'),'decision_live_active')
        self.assertEqual(step['started_when']['key'],'decisions.active')
        self.assertEqual(step['completion'],manual)
