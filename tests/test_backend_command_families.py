from copy import deepcopy
from unittest import TestCase
from hoi4companion.backend_core import _request
from hoi4companion.resolution import CatalogIndex
from hoi4companion._foreign_commands_reference import commands,prefix_commands
from hoi4companion._fortifications_reference import command
from hoi4companion._doctrine_commands_reference import doctrine_command
from hoi4companion._production_sequence_reference import annotate_word_revisions


class NativeCommandFamilyTests(TestCase):
    def test_foreign_clauses_keep_targets_costs_and_local_dependencies(self):
        index=CatalogIndex([],country_names={'CZE':['Czechoslovakia'],'ROM':['Romania'],'ITA':['Italy']})
        for text in ('REL + NA CZE, cancel REL, REL+spy pressure ROM', 'REL with Italy (25PP)',
                     'spy network ROM, spy pressure', 'improve relations with Italy if useful',
                     'spy pressure ROM and invade', 'REL CZE, prepare the front', 'REL', 'REL '+('x'*4100)):
            with self.subTest(text=text[:80]):
                self.assertEqual(_request(0,'import_foreign_commands',text=text,**index.native_import_args()),commands(text,index))
                self.assertEqual(_request(0,'import_foreign_prefix',text=text,**index.native_import_args()),list(prefix_commands(text,index)))

    def test_fortification_goals_require_a_unique_building_and_explicit_location(self):
        index=CatalogIndex([{'kind':'building','id':'bunker','name':'Land Fort','max_level':10},
                            {'kind':'building','id':'coastal_bunker','name':'Coastal Fort','max_level':10}])
        for text in ('BUILD: max forts in each tile at ROM border, then defend',
                     'Build 5 coastal forts in 100', '2 land forts in province 300',
                     '10 forts in Budapest', '0 forts in Berlin', 'Prepare the front'):
            self.assertEqual(_request(0,'import_fortification_command',text=text,**index.native_import_args()),command(text,index))

    def test_doctrine_aliases_currency_tactics_and_qualifiers_match(self):
        index=CatalogIndex([
            {'kind':'doctrine','id':'gbp','name':'Grand Battleplan','domain':'land','track':'main','currency':'army_experience'},
            {'kind':'doctrine','id':'sd','name':'Strategic Destruction','domain':'air','track':'main','currency':'air_experience'},
            {'kind':'advisor','id':'expert','name':'Infantry Expert','slot':'high_command','countries':['TST']}])
        for text in ('GBP Land Doctrine ASAP (100AE)', 'SD Air Doc (50AE, discounted)',
                     'GBP Land Doc + Tactic (100AE)', 'GBP Land Doc (20NE)',
                     'GBP Land Doc (20AE) (infantry BUILD), prepare the front',
                     'GBP Land Doc (20AE) (when ready)',
                     'Infantry Expert (100PP) ASAP: GBP Land Doc (20AE)', 'Unknown Land Doc'):
            with self.subTest(text=text):
                self.assertEqual(_request(0,'import_doctrine_command',text=text,country='TST',**index.native_import_args()),doctrine_command(text,index,'TST'))

    def test_production_revision_offsets_use_source_characters_and_parallel_focus_column(self):
        guide={'tracks':{'focus':[
            {'key':'f1','action':{'kind':'select_focus','entity':{'id':'focus1'}},'evidence':[{'source_id':'s','quote':'First Focus'}]},
            {'key':'f2','action':{'kind':'select_focus','entity':{'id':'focus2'}},'evidence':[{'source_id':'s','quote':'Second Focus'}]}],
            'operations':[{'key':'production','action':{'kind':'manual','params':{'category':'production','production_category':'military','instructions':['MIL: Inf:10']}},'evidence':[{'source_id':'s','quote':'MIL: Inf:10'}]}]},
            'sources':[{'id':'s','type':'file','text':'Café 🚂\n[Table row]\n[Column 1]\nPrepare\n[Column 2]\nTools\n[Column 3]\nFirst Focus\n[Table row]\n[Column 1]\nMIL: Inf:10\n[Column 2]\nRadio\n[Column 3]\nSecond Focus'}]}
        expected=deepcopy(guide);changes=annotate_word_revisions(expected)
        self.assertEqual(_request(0,'import_production_revisions',guide=guide),{'guide':expected,'changes':changes})
