from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from hoi4companion.dlc import DlcInventory
from hoi4companion.research_start import read_research_start


class ResearchStartTests(unittest.TestCase):
    def test_country_history_dates_conditions_mod_overlays_and_unknowns(self):
        with TemporaryDirectory() as directory:
            game, mod = Path(directory)/'game', Path(directory)/'mod'
            for root in (game,mod): (root/'history'/'countries').mkdir(parents=True)
            (game/'common'/'bookmarks').mkdir(parents=True)
            (game/'common'/'bookmarks'/'start.txt').write_text('bookmarks={bookmark={date=1936.1.1.12}}')
            path = mod/'history'/'countries'/'TST - Test.txt'
            (game/'history'/'countries'/'TST - Test.txt').write_text('set_technology={wrong=1}')
            path.write_text('''set_technology={base=1}
                if={limit={has_dlc="Example"} set_technology={dlc=1} else={set_technology={legacy=1}}}
                if={limit={dynamic_condition=yes} set_technology={uncertain=1}}
                1939.1.1={set_technology={later=1 base=0}}''')
            dlcs=DlcInventory(items=[{'name':'Example','status':'enabled'}])
            result=read_research_start(game,[mod],'TST',dlcs,{})
            self.assertEqual(result['date'],'1936.1.1.12')
            self.assertEqual(result['researched'],['base','dlc'])
            self.assertEqual(result['unknown'],['uncertain'])
            self.assertNotIn('wrong',result['researched'])
            later=read_research_start(game,[mod],'TST',dlcs,{},'1939.8.14')
            self.assertEqual(later['researched'],['dlc','later'])
            self.assertIsNone(read_research_start(game,[mod],'UNK',dlcs,{}))

    def test_else_if_unknown_merges_and_invalid_levels_remain_unknown(self):
        with TemporaryDirectory() as directory:
            game=Path(directory); (game/'history'/'countries').mkdir(parents=True)
            (game/'history'/'countries'/'TST - Test.txt').write_text('''
                set_technology={bad=nan}
                if={limit={always=no} set_technology={wrong=1}
                    else_if={limit={not={always=no}} set_technology={correct=1}}
                    else={set_technology={wrong_else=1}}}
                if={limit={unknown=yes} set_technology={same=1}
                    else={set_technology={same=1}}}''')
            result=read_research_start(game,[],'TST',DlcInventory(),{},'1936.1.1')
            self.assertEqual(result['researched'],['correct','same'])
            self.assertEqual(result['unknown'],['bad'])
