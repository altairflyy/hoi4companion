from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion._event_instructions_reference import event_command
from hoi4companion.resolution import CatalogIndex
import test_event_instructions as fixtures


class NativeEventInstructionTests(TestCase):
    def test_catalog_parent_and_unique_answer_are_required_without_outcome_guesses(self):
        phrases=['On event: Go forth…', 'Go forth… (event)',
                 'Event: An invitation; choose: Go forth…',
                 'On An invitation event: choose Go forth…',
                 'Send them back… (event x2)', 'Go forth… (event) (100PP)',
                 'event: Go', 'On events always increase war support',
                 'Go forth… and declare war', 'Event: Missing title; choose: Go forth…',
                 'Send them back', 'Select Gandhi on event trigger', 'JAP war on CHI (WT increase)',
                 'Go forth…', 'Welcome back…', 'Welcome back… (event)', None, 'x'*1025]
        for extra in ([],[{'kind':'event','id':'e.3','name':'Another invitation'},
                         {'kind':'event_option','id':'e.3.a','event_id':'e.3','name':'Go forth to victory!'}]):
            index=CatalogIndex(fixtures.catalog()+extra)
            for country in ('TST','GER',None):
                for text in phrases:
                    with self.subTest(text=text,country=country,ambiguous=bool(extra)):
                        self.assertEqual(_request(0,'import_event_command',text=text,country=country,**index.native_import_args()),
                                         event_command(text,index,country))
