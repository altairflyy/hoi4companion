import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from hoi4companion import jomini_parser as native
from hoi4companion.clausewitz import parse


class EncodingTests(unittest.TestCase):
    def test_windows1252_file_keeps_text_paths_and_trailing_spaces(self):
        text='name="Città € Š " path="C:\\mods\\unità\\n "'
        with tempfile.TemporaryDirectory() as directory:
            file=Path(directory)/'mod.txt'
            file.write_bytes(text.encode('cp1252'))
            node=native.parse_file(file)
        self.assertEqual(node.scalar('name'),'Città € Š ')
        self.assertEqual(node.scalar('path'),'C:\\mods\\unità\\n ')
        self.assertEqual(node.source_encoding,'windows-1252')

    def test_utf8_has_precedence_and_explicit_encoding_is_respected(self):
        data='name="Łódź città"'.encode('utf-8')
        self.assertEqual(native.parse_bytes(data).scalar('name'),'Łódź città')
        self.assertEqual(native.parse_bytes(data).source_encoding,'utf-8')
        with self.assertRaises(ValueError):native.parse_bytes(b'name="\xe0"',encoding='utf-8')
        with self.assertRaises(ValueError):native.parse_bytes(b'\xef\xbb\xbfname="\xe0"')

    def test_binary_and_ambiguous_undefined_bytes_are_not_reinterpreted(self):
        for data in (b'HOI4bin\x82',b'x="\x00"',b'x="\x81"',b'PK\x03\x04'):
            with self.subTest(data=data),self.assertRaises(ValueError):native.parse_bytes(data)
        with self.assertRaises(ValueError):native.parse_bytes(b'x=yes',encoding='guess')

    def test_known_utf8_comment_controls_do_not_become_false_binary_rejections(self):
        self.assertEqual(native.parse_bytes(b'# comment with \x16 control\nx=yes').scalar('x'),'yes')

    def test_focus_loader_reads_legacy_encoded_mod_without_replacement_characters(self):
        from hoi4companion.focus import load_trees
        with tempfile.TemporaryDirectory() as directory:
            file=Path(directory)/'focus.txt'
            file.write_bytes('focus_tree={id=t focus={id=F icon="GFX_città"}}'.encode('cp1252'))
            trees,warnings=load_trees([file],{})
        self.assertFalse(warnings)
        self.assertEqual(trees['t'].focuses['F'].icon,'GFX_città')


class JominiJsonTests(unittest.TestCase):
    def test_pairs_keep_duplicates_order_operators_and_literal_ids(self):
        result=native.export_json('a=001 b=yes a=002 x>=3 text="C:\\mods\\n "')
        self.assertEqual(result['format'],'jomini-json')
        self.assertEqual(result['duplicate_keys'],'key_value_pairs')
        data=json.loads(result['json'])['val']
        self.assertEqual(data[:3],[['a','001'],['b','yes'],['a','002']])
        self.assertEqual(data[3],['x',{'GREATER_THAN_EQUAL':'3'}])
        self.assertEqual(data[-1],['text','C:\\mods\\n '])

    def test_export_has_no_internal_wrapper_or_anonymous_marker(self):
        result=native.export_json('items={one two} {} title=CZE_[COUNTRY]')
        self.assertNotIn('__companion_anonymous_block__',result['json'])
        self.assertNotIn('"root"',result['json'])
        self.assertIn('CZE_[COUNTRY]',result['json'])

    def test_explicit_group_mode_and_encoding(self):
        result=native.export_json('a=1 a=2 name="città"'.encode('cp1252'),duplicate_keys='group',encoding='windows-1252')
        self.assertEqual(json.loads(result['json']),{'a':['1','2'],'name':'città'})
        self.assertEqual(result['source_encoding'],'windows-1252')
        with self.assertRaises(ValueError):native.export_json('a=1',duplicate_keys='last_wins')

    def test_preserve_mode_requires_duplicate_aware_json_reader(self):
        result=native.export_json('a=1 b=2 a=3',duplicate_keys='preserve')
        self.assertEqual(json.loads(result['json'],object_pairs_hook=list),[('a','1'),('b','2'),('a','3')])

    def test_empty_and_anonymous_root_arrays_use_native_json_shapes(self):
        self.assertEqual(json.loads(native.export_json('')['json']),{'type':'array','val':[]})
        self.assertEqual(json.loads(native.export_json('one two')['json']),{'type':'array','val':['one','two']})


class ExportCommandTests(unittest.TestCase):
    def test_command_exports_without_initializing_store_and_refuses_overwrite(self):
        from hoi4companion.__main__ import main
        with tempfile.TemporaryDirectory() as directory:
            source=Path(directory)/'script.txt'
            target=Path(directory)/'script.json'
            original='name="città" a=1 a=2'.encode('cp1252')
            source.write_bytes(original)
            args=['hoi4companion','--export-script',str(source),'--output',str(target)]
            with patch('sys.argv',args),patch('hoi4companion.__main__.Store') as store:
                main()
                store.assert_not_called()
                saved=target.read_bytes()
                self.assertEqual(json.loads(saved)['val'],[['name','città'],['a','1'],['a','2']])
                with self.assertRaises(SystemExit) as error:main()
                self.assertEqual(error.exception.code,1)
                self.assertEqual(target.read_bytes(),saved)
                self.assertEqual(source.read_bytes(),original)

    def test_command_requires_export_for_output_option(self):
        from hoi4companion.__main__ import main
        with patch('sys.argv',['hoi4companion','--output','test.json']),self.assertRaises(SystemExit) as error:
            main()
        self.assertEqual(error.exception.code,2)


class ExpandedTypedModelsTests(unittest.TestCase):
    def test_focus_prerequisite_groups_and_mutex_are_typed_without_losing_unknown_fields(self):
        node=parse('focus_tree={focus={id=A icon=GFX_a prerequisite={focus=B focus=C} prerequisite={focus=D} mutually_exclusive={focus=X focus=X} new={a=b}}}').blocks('focus_tree')[0].blocks('focus')[0]
        self.assertEqual(node.projection['prerequisites'],[['B','C'],['D']])
        self.assertEqual(node.projection['mutually_exclusive'],['X'])
        self.assertEqual(node.projection['icon'],'GFX_a')
        self.assertEqual(node.blocks('new')[0].scalar('a'),'b')
        from hoi4companion.clausewitz import Node
        from hoi4companion.focus import _build_focus
        original=Node.blocks
        def guard(target,key):
            if target is node and key in ('prerequisite','mutually_exclusive'):
                raise AssertionError('Focus relations were re-extracted in Python')
            return original(target,key)
        with patch.object(Node,'blocks',guard):
            focus=_build_focus(node,{},'fixture')
        self.assertEqual(focus.prerequisites,[['B','C'],['D']])
        self.assertEqual(focus.mutually_exclusive,['X'])
