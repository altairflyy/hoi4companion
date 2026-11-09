import copy,json,tempfile,unittest
from pathlib import Path
from hoi4companion.political_import import political_catalog
from hoi4companion.resolution import CatalogIndex
from hoi4companion.guide_runtime import evaluate
import test_informal_operations as informal


DOCTRINES=[{'kind':'doctrine','id':key,'name':name,'aliases':[],'domain':'land','track':track,'currency':'army_experience'}
           for key,name,track in [('grand_battleplan','Grand Battleplan','grand'),('grand_assault','Grand Assault','operations'),
                                 ('assault_infantry','Assault Infantry','infantry'),('streamlined_deployment','Streamlined Deployment','armor')]]


class GuideVocabularyTests(unittest.TestCase):
    def test_character_instances_are_read_and_equivalent_tokens_deduplicated(self):
        with tempfile.TemporaryDirectory() as t:
            p=Path(t)/'common/characters/TST.txt';p.parent.mkdir(parents=True)
            p.write_text('''characters={TST_person={instance={allowed={has_dlc="Example"} name=person advisor={slot=theorist idea_token=TST_theorist allowed={original_tag=TST} traits={military_theorist}}}
              instance={allowed={NOT={has_dlc="Example"}} name=person advisor={slot=theorist idea_token=TST_theorist allowed={original_tag=TST} traits={military_theorist}}}}}''')
            rows,_=political_catalog(Path(t),[],{}, {'person':'Test Person','military_theorist':'Military Theorist'})
        advisors=[r for r in rows if r['kind']=='advisor'];self.assertEqual(len(advisors),1)
        self.assertIn('Military Theorist',advisors[0]['aliases']);self.assertEqual(advisors[0]['countries'],['TST'])
        from hoi4companion.resolution import ResolutionContext
        index=CatalogIndex(rows)
        self.assertEqual(len(index.matches('Military Theorist',('advisor',),ResolutionContext(country='TST'))),1)
        self.assertEqual(index.matches('Military Theorist',('advisor',),ResolutionContext(country='ALT')),[])

    def test_contextual_doctrine_abbreviations_resolve_and_do_not_invent_selection(self):
        report,_=informal.InformalOperationsTests().report(['GBP Land Doc (80AE)','GA Land Doc (80AE)','AI Land Doc (80AE)','SD Land Doc (80AE)'],DOCTRINES)
        rows=report['guide']['tracks']['operations']
        self.assertEqual([r['action']['params'].get('doctrine_id') for r in rows],[r['id'] for r in DOCTRINES])
        self.assertTrue(all(r.get('tracking')!='reminder' for r in rows))
        self.assertEqual(rows[2]['completion']['key'],'doctrines.selected')
        unknown=informal.InformalOperationsTests().state();self.assertEqual(evaluate(report['guide'],unknown)['steps'][0]['status'],'unverifiable')
        selected=informal.InformalOperationsTests().state(**{'doctrines.selected':['grand_battleplan']})
        self.assertEqual(evaluate(report['guide'],selected)['steps'][0]['status'],'completed')

    def test_compound_tactic_stays_separate_and_has_no_invented_total_cost(self):
        report,_=informal.InformalOperationsTests().report(['GBP Land Doc + Tactic (80AE)'],DOCTRINES)
        rows=report['guide']['tracks']['operations']
        self.assertEqual(len(rows),2);self.assertEqual(rows[0]['action']['params']['doctrine_id'],'grand_battleplan')
        self.assertNotIn('resource_costs',rows[0]);self.assertEqual(rows[1]['tracking'],'reminder')
        self.assertIn('Tactic',str(rows[1]['action']['params']['instructions']))

    def test_doctrine_abbreviation_requires_domain_and_ambiguity_is_not_guessed(self):
        for phrase in ('AI','GA','GBP Navy Doc (80NE)','Random Land Doc (80AE)'):
            report,_=informal.InformalOperationsTests().report([phrase],DOCTRINES)
            self.assertTrue(all(r.get('tracking')=='reminder' for r in report['guide']['tracks']['operations']))
        other={**DOCTRINES[0],'id':'other_gbp'}
        report,_=informal.InformalOperationsTests().report(['GBP Land Doc (80AE)'],[*DOCTRINES,other])
        self.assertTrue(all(r.get('tracking')=='reminder' for r in report['guide']['tracks']['operations']))

    def test_same_short_form_has_different_meanings_in_different_doctrine_domains(self):
        air={'kind':'doctrine','id':'strategic_destruction','name':'Strategic Destruction','aliases':[],
             'domain':'air','track':'grand','currency':'air_experience'}
        report,_=informal.InformalOperationsTests().report(['SD Land Doc (80AE)','SD Air Doc (50AE)'],[*DOCTRINES,air])
        rows=report['guide']['tracks']['operations']
        self.assertEqual([r['action']['params']['doctrine_id'] for r in rows],['streamlined_deployment','strategic_destruction'])
        self.assertEqual(rows[1]['resource_costs'],{'country.experience.air':50})

    def test_doctrine_selection_becomes_a_source_order_milestone_and_reimport_is_stable(self):
        equipment={'kind':'equipment','id':'inf','name':'Infantry Equipment','aliases':[],'equipment_ids':['inf']}
        report,sources=informal.InformalOperationsTests().report(['GA Land Doc (80AE)','MIL: Inf:10'],[*DOCTRINES,equipment])
        rows=report['guide']['tracks']['operations'];m=rows[1]['action']['params']['production_milestones']
        self.assertEqual(m[0]['condition'],{'op':'fact','key':'doctrines.selected','test':'contains','value':'grand_assault'})
        from hoi4companion.guide_importer import preview
        self.assertEqual(preview(report['guide'],sources,[*DOCTRINES,equipment])['guide'],report['guide'])

    def test_doctrine_catalog_reads_installed_ids_labels_and_mod_overrides(self):
        from hoi4companion.doctrines import doctrine_catalog
        with tempfile.TemporaryDirectory() as t:
            game=Path(t)/'game';mod=Path(t)/'mod'
            path='common/doctrines/grand_doctrines/land.txt'
            for root,identifier in [(game,'old'),(mod,'new')]:
                p=root/path;p.parent.mkdir(parents=True);p.write_text(identifier+'={folder=land name=LABEL xp_cost=100 xp_type=army}')
            p=game/'common/doctrines/subdoctrines/operations.txt';p.parent.mkdir(parents=True);p.write_text('grand_assault={track=operations name=GA xp_type=army xp_cost=100}')
            rows,issues=doctrine_catalog(game,[mod],{'LABEL':'Tradotto'},{'LABEL':'New Doctrine','GA':'Grand Assault'})
        self.assertFalse(issues);self.assertEqual({r['id'] for r in rows},{'new','grand_assault'})
        self.assertEqual(next(r for r in rows if r['id']=='new')['localized_name'],'Tradotto')
        self.assertEqual(next(r for r in rows if r['id']=='grand_assault')['domain'],'land')

    def test_added_tactic_note_does_not_renumber_the_existing_production_step(self):
        advisor={'kind':'advisor','id':'theorist','character_id':'person','slot':'theorist','name':'Test Theorist','aliases':['Military Theorist'],'countries':['TST']}
        equipment={'kind':'equipment','id':'inf','name':'Infantry Equipment','aliases':[],'equipment_ids':['inf']}
        report,_=informal.InformalOperationsTests().report(['Military Theorist (100PP)\n- GBP Land Doc + Tactic (80AE)\n- MIL: Inf:10'],[*DOCTRINES,advisor,equipment])
        rows=report['guide']['tracks']['operations'];production=next(r for r in rows if r['action']['params'].get('category')=='production')
        self.assertEqual(production['key'],'word-0001--operation-3')
        self.assertTrue(any('--note-' in r['key'] for r in rows))
        self.assertIn('doctrines.selected',[m['condition']['key'] for m in production['action']['params']['production_milestones']])

    def test_generated_milestones_refresh_after_a_previously_unknown_adoption_is_bound(self):
        from test_production_milestones import fixture
        from hoi4companion.production_milestones import annotate
        g=fixture();rows=g['tracks']['operations'];new=copy.deepcopy(rows[0]);new['key']='later_doctrine'
        new['title']='Later Doctrine';new['action']['params']={'category':'army','instructions':['Later Doctrine']}
        new['evidence'][0]['quote']='Later Doctrine';new.pop('completion');new['tracking']='reminder'
        rows.insert(-1,new);g['sources'][0]['text']=g['sources'][0]['text'].replace('MIL:','Later Doctrine\nMIL:')
        annotate(g);new.pop('tracking');new['completion']={'op':'fact','key':'doctrines.selected','test':'contains','value':'grand_assault'}
        self.assertTrue(annotate(g))
        self.assertIn('later_doctrine',[m['step'] for m in rows[-1]['action']['params']['production_milestones']])

    def test_compound_suffixes_preserve_methods_and_accept_asap_discounted_costs(self):
        naval={'kind':'doctrine','id':'trade_interdiction','name':'Trade Interdiction','aliases':[],
               'domain':'naval','track':'grand','currency':'navy_experience'}
        cases=[('TI Naval Doc, then BUILD','trade_interdiction','then BUILD'),
               ('TI Naval Doc + SE Spirit, repair navy','trade_interdiction','SE Spirit, repair navy'),
               ('AI land doc (60AE, discounted), ToS spirit and 3 ALP docs next','assault_infantry','ToS spirit and 3 ALP docs next'),
               ('GBP land doc + tactic ASAP (100AE)','grand_battleplan','Preferred Tactic')]
        for text,identifier,remainder in cases:
            with self.subTest(text=text):
                report,_=informal.InformalOperationsTests().report([text],[*DOCTRINES,naval])
                rows=report['guide']['tracks']['operations'];self.assertEqual(rows[0]['action']['params'].get('doctrine_id'),identifier)
                self.assertIn(remainder,str([r['action']['params'].get('instructions') for r in rows[1:]]))
                self.assertTrue(all(r.get('tracking')=='reminder' for r in rows[1:]))
                if 'discounted' in text:self.assertEqual(rows[0]['resource_costs'],{'country.experience.army':60})

    def test_doctrine_condition_is_not_dropped_when_extracting_a_compound(self):
        for text in ('GA Land Doc if ready','GA Land Doc (if at war)','when ready GA Land Doc',
                     'GA Land Doc instead of AI Land Doc','GA Land Doc or AI Land Doc'):
            report,_=informal.InformalOperationsTests().report([text],DOCTRINES)
            self.assertTrue(all(r.get('tracking')=='reminder' for r in report['guide']['tracks']['operations']),text)

    def test_prefix_appointment_and_doctrine_keep_both_actions_and_original_quote(self):
        advisor={'kind':'advisor','id':'theorist','character_id':'person','slot':'theorist','name':'Test Theorist','aliases':['Military Theorist'],'countries':['TST']}
        text='Military Theorist (100PP) ASAP: GA land doc (INFs BUILD)'
        report,_=informal.InformalOperationsTests().report([text],[*DOCTRINES,advisor])
        rows=report['guide']['tracks']['operations']
        self.assertEqual(rows[0]['action']['params']['advisor_id'],'theorist')
        self.assertEqual(rows[1]['action']['params']['doctrine_id'],'grand_assault')
        self.assertEqual(rows[0]['political_power_cost'],100)
        self.assertIn('INFs BUILD',str(rows[2]['action']['params']['instructions']))
        self.assertTrue(all(r['evidence'][0]['quote']==text for r in rows))
