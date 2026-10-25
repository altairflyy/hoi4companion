import copy
import json
import re
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from hoi4companion.guide_importer import Importer, catalog_entities, preview
from hoi4companion.import_sources import acquire_source, public_url, parse_html, parse_captions
from hoi4companion.guide import load_guide, list_guides
from hoi4companion.guide_runtime import evaluate


def source():
    return acquire_source(text='Research Basic Machine Tools. Build 2 military factories in Berlin.')


def definition(src):
    return {'schema_version': 1, 'id': 'import-test', 'title': 'Imported opening',
            'applies_to': {}, 'sources': [{'id': src['id'], 'type': 'manual'}],
            'tracks': {'research': [{'key': 'tools', 'title': 'Tools',
                'action': {'kind': 'research', 'entity': {'kind': 'technology', 'id': 'Basic Machine Tools'}},
                'evidence': [{'source_id': src['id'], 'basis': 'explicit', 'quote': 'Research Basic Machine Tools.'}]}]}}


ENTITIES = [{'kind': 'technology', 'id': 'basic_machine_tools', 'name': 'Basic Machine Tools', 'aliases': []}]


class ImportTests(unittest.TestCase):
    def test_prompt_monitoring_examples_import_without_manual_gates_and_respect_unknown_state(self):
        prompt=Importer(lambda:[]).prompt()
        match=re.search(r'<!-- esempi-authoring-v1 -->\s*```json\s*(.*?)```',prompt,re.S)
        self.assertIsNotNone(match,'Il prompt deve fornire esempi verificabili di monitoraggio')
        steps=json.loads(match[1]);text='\n'.join(s['instructions'][0] for s in steps)
        src=acquire_source(text=text)
        for step in steps:step['evidence']=[{'source_id':src['id'],'basis':'explicit','quote':step['instructions'][0]}]
        loaded=Importer(lambda:[]).document({'format':'hoi4-guide-authoring','authoring_schema_version':1,'sources':[src],
            'guide':{'id':'prompt-examples','title':'Examples','steps':steps}})
        guide=loaded['guide'];steps=guide['tracks']['operations']
        catalog=[{'kind':'building','id':'arms_factory','name':'Military Factory'},{'kind':'state','id':'123','name':'Example State'},
                 {'kind':'equipment','id':'infantry_equipment_1','name':'Infantry Equipment I'},
                 {'kind':'law','id':'war_economy','name':'War Economy','group':'economy'}]
        report=preview(guide,[src],catalog);self.assertTrue(report['valid'],report['errors'])
        self.assertEqual([s['monitoring'] for s in report['operations']],['automatic','automatic','automatic','reminder','automatic'])
        self.assertEqual(report['operations'][0]['reason'],'baseline_from_live')
        self.assertEqual(report['guide']['tracks']['operations'][1]['action']['params']['requested_factories'],10)
        for step in steps[:3]:self.assertNotIn('completion',step)
        for step in report['guide']['tracks']['operations']:
            if step.get('completion'):self.assertNotEqual(step['completion']['op'],'check')
        isolated=copy.deepcopy(report['guide']);isolated['tracks']['operations']=[isolated['tracks']['operations'][-1]]
        known={'schema_version':1,'usable':True,'facts':{'country.divisions.total':{'value':48,'status':'calculated','source':'fixture'}}}
        self.assertEqual(evaluate(isolated,known)['steps'][0]['status'],'completed')
        self.assertNotEqual(evaluate(isolated,{'schema_version':1,'usable':True,'facts':{}})['steps'][0]['status'],'completed')

    def test_importing_another_guide_keeps_existing_guides(self):
        src = source()
        with TemporaryDirectory() as root:
            importer = Importer(lambda: ENTITIES, root)
            first = definition(src)
            second = copy.deepcopy(first)
            second.update(id='another-country-guide', title='Another guide')
            importer.install(first, [src], reviewed=True)
            importer.install(second, [src], reviewed=True)
            self.assertEqual({g['id'] for g in list_guides(Path(root))}, {first['id'], second['id']})
            self.assertEqual(load_guide(second['id'], directory=Path(root))['title'], second['title'])

    def test_resolves_unique_names_without_mutation_and_reuses_v1(self):
        src = source(); guide = definition(src); before = copy.deepcopy(guide)
        result = preview(guide, [src], ENTITIES)
        self.assertTrue(result['valid'], result)
        self.assertEqual(result['guide']['tracks']['research'][0]['target'], 'basic_machine_tools')
        self.assertEqual(guide, before)
        self.assertNotIn('completion', result['guide']['tracks']['research'][0])

    def test_ambiguity_stays_blocked_and_a_unique_small_typo_resolves(self):
        src = source(); guide = definition(src)
        entities = ENTITIES + [{**ENTITIES[0], 'id': 'other_tools'}]
        report = preview(guide, [src], entities)
        self.assertFalse(report['valid']); self.assertEqual(report['resolutions'][0]['status'], 'ambiguous')
        guide['tracks']['research'][0]['action']['entity']['id'] = 'Basic Machine Tool'
        report = preview(guide, [src], ENTITIES)
        self.assertTrue(report['valid']); self.assertEqual(report['resolutions'][0]['matching_basis'],'minor_typo')

    def test_cycles_missing_evidence_and_invented_quotes_block_install(self):
        src = source(); guide = definition(src)
        for change in ('cycle', 'missing', 'quote'):
            value = copy.deepcopy(guide); row = value['tracks']['research'][0]
            if change == 'cycle': row['depends_on'] = ['tools']
            if change == 'missing': row['evidence'] = []
            if change == 'quote': row['evidence'][0]['quote'] = 'Invented advice'
            self.assertFalse(preview(value, [src], ENTITIES)['valid'], change)

    def test_focus_scope_and_catalog_presence_are_required(self):
        src = source(); guide = definition(src)
        guide['applies_to'] = {'tree_id': 'german'}
        guide['tracks']['research'][0]['action'] = {'kind': 'select_focus', 'entity': {'kind': 'focus', 'id': 'same'}}
        entities = [{'kind': 'focus', 'id': 'same', 'name': 'same', 'scope': 'other', 'aliases': []}]
        self.assertFalse(preview(guide, [src], entities)['valid'])

    def test_file_roundtrip_install_and_no_overwrite(self):
        src = source(); guide = definition(src)
        with TemporaryDirectory() as folder:
            importer = Importer(lambda: ENTITIES, Path(folder))
            with self.assertRaises(ValueError): importer.install(guide, [src], reviewed=False)
            saved = importer.install(guide, [src], reviewed=True)
            self.assertEqual(json.loads(Path(saved['path']).read_text())['id'], guide['id'])
            with self.assertRaises(ValueError): importer.install(guide, [src], reviewed=True)
            guide['id'] = '../escape'
            with self.assertRaises(ValueError): importer.install(guide, [src], reviewed=True)

    def test_packet_contains_source_catalog_and_no_game_state(self):
        src = source(); importer = Importer(lambda: ENTITIES, Path('unused'))
        packet = importer.packet([src])
        self.assertEqual(packet['import_schema_version'], 1)
        self.assertEqual(packet['sources'][0]['text'], src['text'])
        self.assertIn('JSON', packet['instructions'])
        self.assertEqual(packet['catalog'], [{'kind':'technology','id':'basic_machine_tools','name':'Basic Machine Tools'}])
        self.assertEqual(ENTITIES[0]['aliases'], [])
        self.assertNotIn('live', packet)

    def test_ai_packet_keeps_parent_event_and_all_its_options_when_one_answer_is_mentioned(self):
        event={'kind':'event','id':'example.100','name':'A diplomatic invitation'}
        options=[{'kind':'event_option','id':'example.100.a','event_id':'example.100','name':'Accept this invitation'},
                 {'kind':'event_option','id':'example.100.b','event_id':'example.100','name':'Refuse this invitation'}]
        unrelated={'kind':'event','id':'example.200','name':'Unrelated event'}
        importer=Importer(lambda:[*ENTITIES,event,*options,unrelated])
        for text in ('Choose example.100.a.', 'Accept this invitation.'):
            packet=importer.packet([acquire_source(text=text)])
            self.assertEqual({row['id'] for row in packet['catalog']},
                             {'basic_machine_tools','example.100','example.100.a','example.100.b'})
            self.assertEqual([row['event_id'] for row in packet['catalog'] if row['kind']=='event_option'],
                             ['example.100','example.100'])

    def test_prompt_explains_event_parent_link_and_requires_catalog_ids_when_available(self):
        prompt=Importer(lambda:[]).prompt()
        for required in ('kind:"event_option"','event_id','event_option_id','stesso evento','testo completo'):
            self.assertIn(required,prompt)

    def test_standalone_prompt_is_available_before_acquiring_sources(self):
        importer = Importer(lambda: ENTITIES, Path('unused'))
        prompt = importer.prompt()
        for text in ('hoi4-guide-import', 'quantity', 'template', 'research', 'sources', 'sha256'):
            self.assertIn(text, prompt)
        self.assertNotIn('test-local-key', prompt)

    def test_external_file_resolves_source_ids_without_ai_generated_hashes(self):
        src = source(); guide = definition(src)
        src.pop('content_sha256'); src['id'] = 'source-1'
        guide['sources'][0]['id'] = 'source-1'
        guide['tracks']['research'][0]['evidence'][0]['source_id'] = 'source-1'
        file = {'format':'hoi4-guide-import', 'import_schema_version':1, 'sources':[src], 'guide':guide}
        before = copy.deepcopy(file)
        importer = Importer(lambda: ENTITIES, Path('unused'))
        loaded = importer.document(file)
        self.assertEqual(file, before)
        self.assertTrue(loaded['sources'][0]['content_sha256'])
        self.assertTrue(preview(loaded['guide'], loaded['sources'], ENTITIES)['valid'])
        canonical = loaded['sources'][0]['id']
        self.assertEqual(loaded['guide']['tracks']['research'][0]['evidence'][0]['source_id'], canonical)
        self.assertEqual(importer.document(loaded), loaded)

    def test_external_file_rejects_wrong_hashes_unknown_refs_and_duplicate_sources(self):
        src = source(); guide = definition(src)
        importer = Importer(lambda: ENTITIES, Path('unused'))
        valid = {'import_schema_version':1, 'sources':[src], 'guide':guide}
        for change in ('hash', 'version', 'reference', 'duplicate'):
            value = copy.deepcopy(valid)
            if change == 'hash': value['sources'][0]['content_sha256'] = 'wrong'
            if change == 'version': value['import_schema_version'] = 9
            if change == 'reference': value['guide']['sources'][0]['id'] = 'missing'
            if change == 'duplicate': value['sources'].append(copy.deepcopy(src))
            with self.subTest(change=change), self.assertRaises(ValueError): importer.document(value)

    def test_visual_external_observations_stay_declared_evidence_not_live_proof(self):
        src = source(); src.pop('content_sha256'); src['type'] = 'youtube'
        src['segments'] = [{'start':12, 'text':'Research Basic Machine Tools.'}]
        guide = definition(src); evidence = guide['tracks']['research'][0]['evidence'][0]
        evidence.update(basis='visual', timestamp_seconds=12)
        loaded = Importer(lambda: ENTITIES, Path('unused')).document({'import_schema_version':1, 'sources':[src], 'guide':guide})
        report = preview(loaded['guide'], loaded['sources'], ENTITIES)
        self.assertTrue(report['valid']); self.assertTrue(any('visiva' in item for item in report['warnings']))
        self.assertNotIn('completion', report['guide']['tracks']['research'][0])
        loaded['guide']['tracks']['research'][0]['evidence'][0].pop('timestamp_seconds')
        self.assertFalse(preview(loaded['guide'], loaded['sources'], ENTITIES)['valid'])

    def test_strict_payloads_and_source_integrity(self):
        src = source(); guide = definition(src)
        guide['id'] = 'ok'
        for value in (None, [], {'tracks': {'r': [None]}}, {**guide, 'id': 1}, {**guide, 'extra': float('nan')}):
            self.assertFalse(preview(value, [src], ENTITIES)['valid'])
        broken = {**src, 'text': 'Changed source'}
        self.assertFalse(preview(guide, [broken], ENTITIES)['valid'])
        for field, wrong in [('assumptions', None), ('milestones', None), ('milestones', [{'key':'hidden', 'title':'Unsourced advice'}]), ('subtitle', {}), ('source', {'chapters': {}})]:
            self.assertFalse(preview({**guide, field: wrong}, [src], ENTITIES)['valid'], field)

    def test_timestamps_need_actual_segment_and_source_quote(self):
        src = source(); guide = definition(src)
        guide['tracks']['research'][0]['evidence'][0]['timestamp_seconds'] = 5
        self.assertFalse(preview(guide, [src], ENTITIES)['valid'])
        src['segments'] = [{'start': 5, 'text': 'Research Basic Machine Tools.'}]
        self.assertTrue(preview(guide, [src], ENTITIES)['valid'])
        src['segments'][0]['start'] = 12
        self.assertFalse(preview(guide, [src], ENTITIES)['valid'])

    def test_provider_is_optional_and_validates_json_instead_of_installing(self):
        import io
        from urllib.error import HTTPError
        src = source(); guide = definition(src)
        importer = Importer(lambda: ENTITIES, Path('unused'))
        with patch.dict('os.environ', {}, clear=True):
            self.assertFalse(importer.status()['ai_configured'])
            with self.assertRaises(ValueError): importer.extract([src])
        class Response(io.BytesIO):
            def __enter__(self): return self
            def __exit__(self, *args): self.close()
        with patch.dict('os.environ', {'OPENAI_API_KEY': 'test-local-key', 'HOI4_IMPORT_MODEL': 'test-model'}):
            with patch('hoi4companion.guide_importer.build_opener') as opener:
                opener.return_value.open.return_value = Response(json.dumps({'status': 'completed', 'output': [
                    {'type': 'message', 'content': [{'type': 'output_text', 'text': json.dumps(guide)}]}]}).encode())
                self.assertTrue(importer.extract([src])['valid'])
                request = opener.return_value.open.call_args.args[0]
                body = json.loads(request.data)
                self.assertFalse(body['store']); self.assertEqual(body['text']['format']['type'], 'json_object')
                self.assertNotIn('test-local-key', json.dumps(body))
                self.assertNotIn('test-local-key', json.dumps(importer.status()))
            for response in ({'status': 'incomplete'}, {'status': 'completed', 'output': []}):
                with patch('hoi4companion.guide_importer.build_opener') as opener:
                    opener.return_value.open.return_value = Response(json.dumps(response).encode())
                    with self.assertRaises(ValueError): importer.extract([src])
            with patch('hoi4companion.guide_importer.build_opener') as opener:
                opener.return_value.open.side_effect = HTTPError('https://api.openai.com', 401, 'test-local-key', {}, None)
                with self.assertRaises(ValueError) as error: importer.extract([src])
                self.assertNotIn('test-local-key', str(error.exception))

    def test_youtube_acquisition_description_captions_and_missing_subtitles(self):
        metadata = {'title': 'Guide', 'description': 'Research tools.', 'subtitles': {'en': [
            {'ext': 'json3', 'url': 'https://captions.example/captions'}]}}
        def run(args, **kwargs):
            self.assertIn('--skip-download', args); self.assertIn('--ignore-config', args)
            self.assertNotIn('--cookies-from-browser', args)
            kwargs['stdout'].write(json.dumps(metadata).encode())
            return SimpleNamespace(returncode=0)
        captions = json.dumps({'events': [{'tStartMs': 5000, 'segs': [{'utf8': 'Research tools'}]}]}).encode()
        with patch('hoi4companion.import_sources.ytdlp_path', return_value='yt-dlp'), patch('hoi4companion.import_sources.subprocess.run', side_effect=run), patch('hoi4companion.import_sources.fetch_public', return_value=(captions, 'application/json', 'url')):
            value = acquire_source(url='https://youtu.be/FBPdhlwqMwM')
            self.assertEqual(value['segments'][0]['start'], 5)
            self.assertIn('Research tools.', value['text'])
            metadata.pop('subtitles')
            value = acquire_source(url='https://www.youtube.com/watch?v=FBPdhlwqMwM')
            self.assertEqual(value['segments'], [])
            self.assertTrue(any('Trascrizione assente' in text for text in value['warnings']))

    def test_web_static_text_mime_and_limits(self):
        with patch('hoi4companion.import_sources.fetch_public', return_value=(b'<h1>Research tools</h1><script>bad()</script>', 'text/html', 'https://example.com/')):
            value = acquire_source(url='https://example.com/')
            self.assertIn('Research tools', value['text']); self.assertNotIn('bad()', value['text'])
        with patch('hoi4companion.import_sources.fetch_public', return_value=(b'pdf', 'application/pdf', 'https://example.com/')):
            with self.assertRaises(ValueError): acquire_source(url='https://example.com/')

    def test_local_guide_catalog_is_additive_and_keeps_bundled_default(self):
        src = source()
        with TemporaryDirectory() as folder, patch('hoi4companion.guide.LOCAL_GUIDES', Path(folder)):
            importer = Importer(lambda: ENTITIES, Path(folder))
            value = definition(src); value['default'] = True
            importer.install(value, [src], reviewed=True)
            self.assertIn('import-test', {item['id'] for item in list_guides()})
            self.assertEqual(load_guide('import-test')['tracks']['research'][0]['target'], 'basic_machine_tools')
            self.assertEqual(load_guide()['id'], 'poland-wojtek-FBPdhlwqMwM')

    def test_text_and_html_caption_sources_are_bounded_and_not_executed(self):
        self.assertNotIn('evil', parse_html('<main><h1>Guide</h1><script>evil()</script><p>Build tools</p></main>'))
        self.assertEqual(parse_captions({'events': [{'tStartMs': 5000, 'segs': [{'utf8': 'Research tools'}]}]})[0]['start'], 5)
        with self.assertRaises(ValueError): acquire_source(text='x' * 200001)

    def test_only_public_http_sources_no_local_or_credentials(self):
        for url in ('file:///etc/passwd', 'http://127.0.0.1/', 'http://[::1]/', 'https://u:p@example.com', 'http://localhost/'):
            with self.subTest(url=url), self.assertRaises(ValueError): public_url(url)
        with patch('hoi4companion.import_sources.socket.getaddrinfo', return_value=[(2, 1, 6, '', ('192.168.0.2', 443))]):
            with self.assertRaises(ValueError): public_url('https://example.com/')


if __name__ == '__main__': unittest.main()
