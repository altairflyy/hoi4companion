"""Catalog source projections preserve script semantics, text and geometry."""
from dataclasses import asdict
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.clausewitz import parse,Node
from hoi4companion.tooltips import localise,focus_tooltips
from hoi4companion.focus_criteria import compile_rules
from hoi4companion._focus_reference import load_trees

def encode(node):
    if not isinstance(node,Node):return node
    return {'entries':[[key,encode(value)] for key,value in node.entries],**({'projection':node.projection} if node.projection else {})}

class BackendFocusSourceTests(unittest.TestCase):
    def test_deep_valid_script_is_not_lost_to_an_intermediate_json_depth_limit(self):
        with TemporaryDirectory() as directory:
            source=Path(directory)/'deep.txt'
            source.write_text('focus_tree={id=deep focus={id=valid completion_reward={'+('if={'*55)+'add_political_power=1'+('}'*55)+'}}}',encoding='utf8')
            trees,warnings=load_trees([source],{})
            self.assertIn('valid',trees['deep'].focuses)
            result=_request(0,'focus_trees',files=[str(source)],labels={})
            self.assertEqual(result['warnings'],warnings)
            self.assertIn('valid',result['trees']['deep']['focuses'])

    def test_static_localisation_budgets_cycles_escaping_and_numeric_tooltips(self):
        labels={'A':'$B$','B':'$A$','TITLE':'$NAME|G$','NAME':'Name'}
        for text in ('$TITLE$ $VALUE$',r'\$NAME$','$A$','$NAME$ '*600,'§Y$missing$§!'):
            self.assertEqual(_request(0,'focus_localise',text=text,labels=labels),localise(text,labels))
        node=parse('available={custom_trigger_tooltip={tooltip=TITLE has_war=yes}} completion_reward={add_stability=0.025 add_manpower=-100 add_war_support=1e-9 add_political_power=+1e3 if={custom_effect_tooltip=TITLE}}')
        self.assertEqual(_request(0,'focus_tooltips',node=encode(node),labels=labels),focus_tooltips(node,labels))

    def test_criteria_keeps_unknown_triggers_and_exact_operators(self):
        node=parse('available={has_war_support > 0.84 OR={has_tech=a has_idea=b} NOT={has_war=yes has_idea=c} custom_trigger_tooltip={tooltip=NAME has_stability > 0.5} dynamic=yes} allow_branch={original_tag=HUN}')
        self.assertEqual(_request(0,'focus_rules',node=encode(node)),compile_rules(node))

    def test_shared_roots_descendants_ancestors_and_coordinates(self):
        with TemporaryDirectory() as directory:
            source=Path(directory)/'focus.txt'
            source.write_text('''shared_focus={id=parent x=2 y=1}
            shared_focus={id=root prerequisite={focus=parent} x=3 y=2 relative_position_id=parent}
            shared_focus={id=child prerequisite={focus=root} x=1 y=1 relative_position_id=root}
            shared_focus={id=sibling prerequisite={focus=parent}}
            focus_tree={id=one default=yes country={factor=1} shared_focus=root focus={id=own x=2 y=4 cost=5 mutually_exclusive={focus=child}}}
            focus_tree={id=two shared_focus=parent}''',encoding='utf8')
            labels={'own':'Own focus'}
            trees,warnings=load_trees([source],labels)
            result=_request(0,'focus_trees',files=[str(source)],labels=labels)
            self.assertEqual(result['warnings'],warnings)
            for identity,tree in trees.items():
                actual=result['trees'][identity]
                self.assertEqual(actual['country_selector'],encode(tree.country_selector))
                self.assertEqual(actual['focuses'],{key:asdict(value) for key,value in tree.focuses.items()})
                self.assertEqual(actual['shared'],tree.shared)
