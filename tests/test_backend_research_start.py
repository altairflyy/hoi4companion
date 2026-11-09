from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase
from dataclasses import asdict
from hoi4companion.backend_core import _request
from hoi4companion.dlc import DlcInventory
from hoi4companion._research_start_reference import read_research_start


class NativeResearchStartTests(TestCase):
    def test_dated_mod_history_and_unknown_branches_match_without_live_or_save_reads(self):
        with TemporaryDirectory() as directory:
            game=Path(directory)/'game';mod=Path(directory)/'mod'
            for root in (game,mod): (root/'history/countries').mkdir(parents=True)
            (game/'common/bookmarks').mkdir(parents=True)
            (game/'common/bookmarks/start.txt').write_text('bookmarks={bookmark={date=1936.1.1.12}}')
            (game/'history/countries/TST - Test.txt').write_text('set_technology={wrong=1}')
            (mod/'history/countries/TST - Test.txt').write_text('''set_technology={base=1 bad=nan}
                if={limit={has_dlc="Example"} set_technology={dlc=1} else={set_technology={legacy=1}}}
                if={limit={unknown=yes} set_technology={uncertain=1 same=1} else={set_technology={same=1}}}
                if={limit={always=no} set_technology={wrong=1}
                    else_if={limit={not={always=no}} set_technology={correct=1}}}
                unsupported_scope={set_technology={scoped=1}}
                1939.1.1={set_technology={later=1 base=0}}''')
            for dlcs in (DlcInventory(),DlcInventory(items=[{'name':'Example','status':'enabled'}])):
                for country,date in (('TST',None),('TST','1939.8.14'),('UNK',None),('bad',None)):
                    self.assertEqual(_request(0,'source_research_start',game=str(game),mods=[str(mod)],country=country,start_date=date,dlcs=asdict(dlcs),scripts={}),read_research_start(game,[mod],country,dlcs,{},date))
