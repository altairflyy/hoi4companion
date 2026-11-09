import unittest

from hoi4companion.event_instructions import event_command
from hoi4companion.resolution import CatalogIndex
import test_informal_operations as helpers


def catalog():
    return [
        {'kind':'event','id':'e.1','name':'An invitation','country_condition':{'op':'tag','value':'TST'}},
        {'kind':'event_option','id':'e.1.a','event_id':'e.1','name':'Go forth Archduke! We will keep your seat warm.'},
        {'kind':'event_option','id':'e.1.b','event_id':'e.1','name':'We must decline this invitation.'},
        {'kind':'event','id':'e.2','name':'The delegates return','country_condition':{'op':'tag','value':'TST'}},
        {'kind':'event_option','id':'e.2.a','event_id':'e.2','name':'Send them back.'}]


class EventInstructionTests(unittest.TestCase):
    def test_explicit_event_and_unique_abbreviated_button_keep_the_native_parent(self):
        index=CatalogIndex(catalog())
        for text in ('On event: Go forth…','Go forth… (event)',
                     'Event: An invitation; choose: Go forth…',
                     'On An invitation event: choose Go forth…'):
            with self.subTest(text=text):
                result=event_command(text,index,'TST')
                self.assertEqual((result['params']['event_id'],result['params']['event_option_id']),('e.1','e.1.a'))
        self.assertEqual(event_command('Send them back… (event x2)',index,'TST')['params']['event_option_id'],'e.2.a')

    def test_ambiguity_short_prose_unknown_parent_and_national_mismatch_are_not_guessed(self):
        index=CatalogIndex(catalog()+[{'kind':'event','id':'e.3','name':'Another invitation'},
            {'kind':'event_option','id':'e.3.a','event_id':'e.3','name':'Go forth to victory!'}])
        self.assertIsNone(event_command('Go forth… (event)',index,'TST'))
        scoped=event_command('Event: An invitation; choose: Go forth…',index,'TST')
        self.assertEqual(scoped['params']['event_option_id'],'e.1.a')
        for text in ('event: Go','On events always increase war support','Refuse kings until Anastasia',
                     'Go forth… and declare war','Event: Missing title; choose: Go forth…'):
            self.assertIsNone(event_command(text,index,'TST'))
        self.assertIsNone(event_command('Event: An invitation; choose: Go forth…',index,'GER'))
        self.assertIsNone(event_command('Go forth… (event)',CatalogIndex(catalog()[1:]),'TST'))

    def test_word_import_promotes_events_before_generic_timing_and_is_idempotent(self):
        helper=helpers.InformalOperationsTests()
        report,sources=helper.report(['On event: Go forth…','On events: always increase war support'],catalog())
        row=report['guide']['tracks']['operations'][0]
        self.assertEqual(row['action']['params']['event_option_id'],'e.1.a')
        self.assertNotIn('when',row)
        self.assertEqual(row['started_when']['key'],'events.visible_options')
        self.assertNotIn('completion',row)
        self.assertEqual(report['guide']['tracks']['operations'][1]['tracking'],'reminder')
        self.assertIn('On event: Go forth…',row['evidence'][0]['quote'])
        from hoi4companion.guide_importer import preview
        self.assertEqual(preview(report['guide'],sources,catalog())['guide'],report['guide'])

    def test_existing_dependencies_and_authored_completion_are_preserved(self):
        helper=helpers.InformalOperationsTests()
        gate={'op':'check','key':'explicit_gate'};done={'op':'fact','key':'events.executed','test':'contains','value':'e.1.a'}
        def edit(g):g['tracks']['operations'][0].update(when=gate,depends_on=[])
        report,_=helper.report(['Go forth… (event)'],catalog(),edit=edit)
        row=report['guide']['tracks']['operations'][0]
        self.assertEqual(row['when'],gate);self.assertEqual(row['depends_on'],[])
        from hoi4companion.political_import import bind_political
        row['completion']=done;bind_political(row,CatalogIndex(catalog()),'TST')
        self.assertEqual(row['completion'],done)

    def test_plain_focus_or_other_command_does_not_become_an_event(self):
        index=CatalogIndex(catalog()+[{'kind':'focus','id':'f','name':'Send them back','scope':'tree'}])
        self.assertIsNone(event_command('Send them back',index,'TST'))
        self.assertIsNone(event_command('Select Gandhi on event trigger',index,'TST'))
        self.assertIsNone(event_command('JAP war on CHI (WT increase)',index,'TST'))

    def test_short_bare_phrases_do_not_bind_unrelated_global_events(self):
        index=CatalogIndex([{'kind':'event','id':'foreign.1','name':'A return'},
            {'kind':'event_option','id':'foreign.1.a','event_id':'foreign.1','name':'Welcome back!'}])
        self.assertIsNone(event_command('Welcome back…',index,'GER'))
        self.assertEqual(event_command('Welcome back… (event)',index,'GER')['params']['event_option_id'],'foreign.1.a')

    def test_short_native_national_phrase_requires_a_positive_script_identity_hint(self):
        entries=[{'kind':'event','id':'e.1','name':'A march','identity_countries':['TST']},
            {'kind':'event_option','id':'e.1.a','event_id':'e.1','name':'Disperse them and arrest their leader!'}]
        index=CatalogIndex(entries)
        self.assertEqual(event_command('Disperse them…',index,'TST')['params']['event_option_id'],'e.1.a')
        self.assertIsNone(event_command('Disperse them…',index,'GER'))
        self.assertIsNone(event_command('Disperse them…',index,None))

    def test_identity_hint_ignores_other_scopes_or_negated_or_alternative_tags(self):
        from hoi4companion.event_country import recipient_identity_tags
        from hoi4companion.clausewitz import Node
        def hints(entries):return recipient_identity_tags(Node([('trigger',Node(entries))]))
        self.assertEqual(hints([('original_tag','TST')]),['TST'])
        self.assertEqual(hints([('AND',Node([('tag','TST')]))]),['TST'])
        for block in [('FROM',Node([('tag','TST')])),('OR',Node([('tag','TST'),('tag','GER')])),('NOT',Node([('tag','TST')]))]:
            self.assertEqual(hints([block]),[])
        self.assertEqual(hints([('tag','TST'),('original_tag','GER')]),[])

    def test_imported_answer_reaches_the_existing_hud_watch_without_inferring_execution(self):
        from hoi4companion.guide_runtime import evaluate
        from hoi4companion.screen_overlay import targets
        report,_=helpers.InformalOperationsTests().report(['Go forth… (event)'],catalog())
        g=report['guide']
        state={'usable':True,'sample':{'country':'TST','session_id':'s','country_overview':{'country':'TST','events':{'status':'unavailable'}}}}
        runtime=evaluate(g,state);prepared=targets(g,runtime,catalog(),state,prepare_events=True)
        self.assertEqual([(p['event_id'],p['option_id']) for p in prepared],[('e.1','e.1.a')])
        self.assertFalse(prepared[0]['live_visible'])
        for visible in ([{'event_id':'e.1','instance_id':'abc','option_ids':['e.1.a']}],[]):
            state['sample']['country_overview']['events']={'status':'observed','visible':visible}
            runtime=evaluate(g,state)
            self.assertNotEqual(runtime['steps'][0]['status'],'completed')
            self.assertIsNone(runtime['steps'][0]['completion']['value'])


if __name__=='__main__':unittest.main()
