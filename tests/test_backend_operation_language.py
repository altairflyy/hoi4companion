import io
from copy import deepcopy
from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion.resolution import CatalogIndex
from hoi4companion.word_guides import prepare_document
from hoi4companion.guide_authoring import compile_document
from hoi4companion._informal_operations_reference import promote_operations
from test_word_guides import word
import test_informal_operations as operations
import test_informal_families as families
import test_event_instructions as events


class NativeOperationLanguageTests(TestCase):
    def test_complete_source_blocks_preserve_residuals_policies_stable_keys_and_dependencies(self):
        entries=operations.InformalOperationsTests().catalog()+operations.InformalOperationsTests().laws()+families.InformalFamiliesTests().catalog()+events.catalog()+[
            {'kind':'project','id':'project','name':'Test Project'},
            {'kind':'building','id':'bunker','name':'Land Fort','max_level':10},
            {'kind':'doctrine','id':'gbp','name':'Grand Battleplan','domain':'land','track':'main','currency':'army_experience'}]
        index=CatalogIndex(entries,country_names={'CZE':['Czechoslovakia'],'ROM':['Romania']})
        blocks=[['Create Agency & Setup Done'],['Create Agency (spy pressure ROM, prepare the front)'],
                ['At 200PP: War Economy (150PP)\n- Limited Conscription (127PP)\n- Keep training'],
                ['REL + NA CZE, cancel REL, REL+spy pressure ROM, then defend'],
                ['Start special project: Test Project, then War Economy (150PP), then Limited Conscription (150PP)'],
                ['MIL: Sup:1, Inf:ALL'],['Go forth… (event) (100PP)'],
                ['Army Offense (100PP) (before event)'],['GBP Land Doctrine ASAP (100AE)'],
                ['BUILD: max forts in each tile at ROM border, defend behind the river'],
                ['When ready: Infantry Expert (100PP)\n- Monarchist Jurist (150PP)']]
        for instructions in blocks:
            packet=prepare_document(io.BytesIO(word([instructions,[],[]])))
            guide=compile_document(packet['document'])['guide'];guide['applies_to']={'country':'TST'}
            with self.subTest(instructions=instructions):
                expected=deepcopy(guide);changes=promote_operations(expected,index)
                actual=_request(0,'import_promote_operations',guide=guide,library=index.reference_designs,**index.native_import_args())
                self.assertEqual(actual,{'guide':expected,'changes':changes})
                once=deepcopy(expected);again=promote_operations(once,index)
                self.assertEqual(_request(0,'import_promote_operations',guide=expected,library=index.reference_designs,**index.native_import_args()),{'guide':once,'changes':again})
