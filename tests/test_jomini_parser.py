import os
import unittest
from unittest.mock import patch

from hoi4companion.clausewitz import parse
from hoi4companion.jomini_parser import ParserUnavailableError, backend_info
from parser_reference import parse_legacy


class ParserSelectionTests(unittest.TestCase):
    def test_native_library_is_required_without_silent_downgrade(self):
        with patch.dict(os.environ, {'HOI4_JOMINI_LIBRARY':'missing-parser-library.dll'}):
            with self.assertRaises(ParserUnavailableError):
                parse('x=yes')
        with patch.dict(os.environ, {'HOI4_PARSER':'legacy'}):
            with self.assertRaisesRegex(ParserUnavailableError, 'HOI4_PARSER'):
                parse('x=yes')

    def test_default_backend_is_native_without_environment_configuration(self):
        with patch.dict(os.environ):
            os.environ.pop('HOI4_PARSER', None)
            os.environ.pop('HOI4_JOMINI_LIBRARY', None)
            self.assertEqual(backend_info(), {'engine':'jomini','engine_version':'0.37.2','schema_version':1,
                                             'document_model':'TextTape','typed_models':2,'capabilities':['windows-1252','jomini-json','typed-focus-relations']})
            self.assertEqual(parse('x=yes').scalar('x'), 'yes')

    def test_unavailable_backend_aborts_before_catalogs_or_progress_initialisation(self):
        from hoi4companion.api import Store
        with patch('hoi4companion.jomini_parser._package_library', side_effect=ParserUnavailableError('missing')), \
                patch('hoi4companion.api.RunStore') as progress, patch.dict(os.environ):
            os.environ.pop('HOI4_JOMINI_LIBRARY', None)
            with self.assertRaises(ParserUnavailableError):
                Store()
            progress.assert_not_called()

    def test_incompatible_abi_is_a_deployment_error(self):
        from unittest.mock import MagicMock
        from hoi4companion.jomini_parser import _library
        dll=MagicMock()
        dll.hoi4_parser_schema_version.return_value=99
        with patch('hoi4companion.jomini_parser.ctypes.CDLL',return_value=dll):
            with self.assertRaisesRegex(ParserUnavailableError, 'ABI'):
                _library(str(__file__))


class NativeParserTests(unittest.TestCase):
    def native(self, source):
        with patch.dict(os.environ, {'HOI4_PARSER':'jomini'}):
            return parse(source)

    def test_order_duplicate_prerequisites_literals_and_strings(self):
        source='prerequisite={focus=A focus=B} prerequisite={focus=C} x >= 3 values={yes 001 1936.1.1} path="C:\\mods\\a\\n " name="Łódź"'
        self.assertEqual(self.native(source), parse_legacy(source))
        self.assertEqual(len(self.native(source).blocks('prerequisite')), 2)

    def test_quoted_syntax_characters_do_not_turn_into_structure(self):
        value=self.native('x="{" y="=" z="}"')
        self.assertEqual([item for _,item in value.entries], ['{','=','}'])

    def test_invalid_input_has_no_partial_result(self):
        for source in ('x={a=1', 'x=}', 'x="unterminated', 'x=', '}'):
            with self.subTest(source=source), self.assertRaises(ValueError):
                self.native(source)
        with self.assertRaises(ValueError):
            self.native('x={'*181+'}'*181)

    def test_parallel_calls_own_their_results(self):
        from concurrent.futures import ThreadPoolExecutor
        from hoi4companion.jomini_parser import parse as native_parse
        with ThreadPoolExecutor(max_workers=4) as pool:
            values=list(pool.map(native_parse, [f'key="item-{i}"' for i in range(40)]))
        self.assertEqual([v.scalar('key') for v in values], [f'item-{i}' for i in range(40)])

    def test_dynamic_localisation_and_meta_effect_ids_are_not_truncated(self):
        value=self.native('title=[ROOT.GetName] id=CZE_trade_[COUNTRY] cost=var?50')
        self.assertEqual(value.scalar('title'),'[ROOT.GetName]')
        self.assertEqual(value.scalar('id'),'CZE_trade_[COUNTRY]')
        self.assertEqual(value.scalar('cost'),'var?50')

    def test_mixed_containers_ghost_blocks_and_marker_collisions_keep_every_entry(self):
        for source in ('x={a=1 empty={} two x=2 three}', 'a={} {} b={x=1}',
                       '__companion_anonymous_block__=real {} x={"one" two a=1 three}'):
            with self.subTest(source=source):
                self.assertEqual(self.native(source),parse_legacy(source))

    def test_typed_focus_fields_preserve_first_value_operators_expressions_and_unknown_fields(self):
        source='focus_tree={id=t focus={id="A\\\\B " id=later x=2 x=3 y>=9 cost=@cost unknown={x=1}}}'
        node=self.native(source).blocks('focus_tree')[0].blocks('focus')[0]
        self.assertEqual(node.projection,{'kind':'focus','id':'A\\B ','x':'2','y':None,
                                         'cost':'@cost','relative_position_id':None,'icon':None,'prerequisites':[],'mutually_exclusive':[]})
        self.assertEqual(node.blocks('unknown')[0].scalar('x'),'1')
        self.assertEqual(node.all('id'),['A\\B ','later'])
        self.assertEqual(node.scalar('y >='),'9')

    def test_focus_extractor_uses_jomini_typed_fields(self):
        from hoi4companion.clausewitz import Node
        from hoi4companion.focus import _build_focus
        node=self.native('focus_tree={focus={id=A x=2 y=3 cost=5}}').blocks('focus_tree')[0].blocks('focus')[0]
        original=Node.scalar
        def guard(target,key,*args):
            if target is node and key in {'id','x','y','cost','relative_position_id'}:
                raise AssertionError('Typed source field was re-extracted in Python')
            return original(target,key,*args)
        with patch.object(Node,'scalar',guard):
            focus=_build_focus(node,{},'fixture')
        self.assertEqual((focus.id,focus.x,focus.y,focus.cost),('A',2,3,5))

    def test_typed_technology_fields_preserve_variables_and_first_block_masking(self):
        node=self.native('technologies={tech={research_cost=@cost start_year={} start_year=1936 is_special_project_tech=yes other={x=1}}}').blocks('technologies')[0].blocks('tech')[0]
        self.assertEqual(node.projection,{'kind':'technology','research_cost':'@cost','start_year':None,
                                         'is_special_project_tech':'yes'})
        self.assertEqual(node.blocks('other')[0].scalar('x'),'1')

    def test_deepest_accepted_document_remains_supported(self):
        self.native('x={'*180+'}'*180)

    def test_technology_loader_uses_typed_source_fields_and_preserves_variable_resolution(self):
        import tempfile
        from pathlib import Path
        from hoi4companion.clausewitz import Node
        from hoi4companion.technologies import load_technologies
        original=Node.scalar
        def guard(target,key,*args):
            if target.projection and target.projection['kind']=='technology' and key in {'research_cost','start_year','is_special_project_tech'}:
                raise AssertionError('Typed technology field was re-extracted in Python')
            return original(target,key,*args)
        with tempfile.TemporaryDirectory() as directory:
            file=Path(directory)/'tech.txt'
            file.write_text('@cost=1.5 technologies={tech={research_cost=@cost start_year=1936 is_special_project_tech=yes folder={name=test position={x=1 y=2}}}}',encoding='utf-8')
            tags=Path(directory)/'tags.txt'
            tags.write_text('technology_folders={test={}}',encoding='utf-8')
            with patch.object(Node,'scalar',guard):
                catalog=load_technologies([file],[tags],{},None,{})
        self.assertFalse(catalog['warnings'])
        tech=catalog['technologies']['tech']
        self.assertEqual((tech['research_cost'],tech['start_year'],tech['special_project']),(1.5,1936,True))
