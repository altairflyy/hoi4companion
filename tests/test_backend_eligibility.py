"""Static DLC/country predicates must preserve unknown and alternatives."""
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.clausewitz import parse
from hoi4companion.dlc import DlcInventory
from hoi4companion._eligibility_reference import condition,filter_trees
from hoi4companion.focus import Tree
from hoi4companion.script_document import encode

class BackendEligibilityTests(unittest.TestCase):
    def test_predicates_scripts_and_dlc_unicode_match_reference(self):
        scripts={'old':parse('has_dlc="Öld - Pack"'),'cycle':parse('cycle=yes')}
        dlcs=DlcInventory(items=[{'name':'Öld - Pack','status':'enabled'}],complete=False)
        for query in ('old=yes','has_dlc="old - pack"','has_dlc="Unknown"','OR={old=no tag=HUN}',
                      'NOT={tag=GER tag=HUN}','NOR={tag=GER always=no}','NAND={always=yes tag=HUN}',
                      'cycle=yes','runtime=yes','original_tag=HUN'):
            node=parse(query)
            native=_request(0,'source_condition',node=encode(node),country='HUN',dlcs={'items':dlcs.items,'complete':dlcs.complete},scripts={key:encode(value) for key,value in scripts.items()})
            self.assertIs(native,condition(node,'HUN',dlcs,scripts),query)

    def test_global_selector_priority_keeps_unknown_and_ties_unselected(self):
        dlcs=DlcInventory(items=[{'name':'Pack','status':'enabled'}])
        for selectors in (
            {'base':'factor=1','new':'factor=0 modifier={add=10 tag=HUN has_dlc="Pack"}'},
            {'a':'factor=1','b':'factor=1'},
            {'a':'factor=1','b':'factor=0 modifier={add=10 runtime=yes}'},
        ):
            trees={key:Tree(id=key,source=key+'.txt',country_selector=parse(query)) for key,query in selectors.items()}
            visible,excluded=filter_trees(trees,dlcs,{})
            result=_request(0,'source_filter_trees',trees={key:{'id':key,'source':tree.source,'country_selector':encode(tree.country_selector),'eligible_countries':[]} for key,tree in trees.items()},dlcs={'items':dlcs.items,'complete':dlcs.complete},scripts={})
            self.assertEqual(result['visible'],list(visible))
            self.assertEqual(result['excluded'],excluded)
            self.assertEqual(result['countries'],{key:tree.eligible_countries for key,tree in visible.items()})
