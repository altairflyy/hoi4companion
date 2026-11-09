from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion.resolution import CatalogIndex
from hoi4companion._informal_targets_reference import named_command, production_command
from hoi4companion._action_commands_reference import sequence
import test_informal_families as families
import test_informal_operations as operations


class NativeInformalTargetTests(TestCase):
    def test_compound_sequences_preserve_then_start_and_completion(self):
        index=CatalogIndex(operations.InformalOperationsTests().catalog()+operations.InformalOperationsTests().laws()+[
            {'kind':'project','id':'project','name':'Test Project'}])
        for text in ('Start special project: Test Project, then War Economy (150PP), then Limited Conscription (150PP)',
                     'Complete project: Test Project then Infantry Expert (100PP)',
                     'War Economy (150PP) & Limited Conscription',
                     'War Economy & prepare the front', 'War Economy or Limited Conscription',
                     'Start project: Missing then War Economy', 'Infantry Expert (100PP) (before event) then War Economy'):
            with self.subTest(text=text):
                self.assertEqual(_request(0,'import_action_sequence',text=text,country='TST',**index.native_import_args()),
                                 sequence(text,index,'TST'))
    def test_named_roles_spirits_decisions_and_costs_match_without_choosing_a_country(self):
        entries=families.InformalFamiliesTests().catalog()+operations.InformalOperationsTests().catalog()
        index=CatalogIndex(entries)
        for country in ('TST','ALT',None):
            for text in ('Army Offense (100PP) (before event)', 'ASAP: PL Spirit (20AE)',
                         'Political Loyalty (20 NE)', 'Political Loyalty (20 AFE)',
                         'Infantry Expert (100PP)', 'Monarchist Jurist (150PP)',
                         'Restore the Regional Union', 'Unknown (20PP)', 'Army Offense & Something'):
                with self.subTest(text=text,country=country):
                    self.assertEqual(_request(0,'import_named_command',text=text,country=country,**index.native_import_args()),
                                     named_command(text,index,country))

    def test_allocations_shared_counts_remaining_priority_and_residuals_match(self):
        index=CatalogIndex(families.InformalFamiliesTests().catalog())
        for text in ('MIL: Sup:1, Inf:10', 'Production: Sup Equip:1, Inf Equip:10',
                     'MIL: Sup:1, Inf:ALL', 'NAV: Inf:ALL', 'MIL: Inf:ALL, Sup:1',
                     'MIL: Sup, Inf:2', 'MIL: Sup:1, Inf:1 -> 10',
                     'MIL: Inf:10 -> 1', 'MIL: Inf:10 (top priority), Sup:1',
                     'MIL: Sup:1, Inf:10 (both top priority)', 'MIL: Sup:1 (MIO), Inf:10',
                     'Design & produce Inf:10', 'Inf:10', 'MIL: Inf:10, Unknown:1',
                     'MIL: Inf:10, Inf:3', 'MIL: Inf:10 & Sup:1', 'MIL: 3', 'Prepare the front'):
            with self.subTest(text=text):
                self.assertEqual(_request(0,'import_production_command',text=text,country='TST',library=index.reference_designs,declared=None,**index.native_import_args()),
                                 production_command(text,index,'TST'))
