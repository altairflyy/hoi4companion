import json
from pathlib import Path
from tempfile import TemporaryDirectory
from threading import RLock, Thread
from http.server import ThreadingHTTPServer
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from unittest import TestCase
from unittest.mock import Mock, patch

from hoi4companion.api import Store, make_handler
from hoi4companion.i18n import localize_html, localize_payload, translate


class LocalizationTests(TestCase):
    def test_default_diagnostics_and_formatted_arguments(self):
        self.assertEqual(translate('Gioco non avviato'),'Game is not running')
        self.assertEqual(translate('Game is not running','italian'),'Gioco non avviato')
        self.assertEqual(translate('Lettura GUI sospesa: GER_focus <tag>'),
                         'GUI reading suspended: GER_focus <tag>')
        self.assertEqual(translate('POL_unknown_id'),'POL_unknown_id')

    def test_guide_evidence_and_names_are_never_translated(self):
        payload={'error':'Lettura parziale','name':'Pausa','id':'GER_focus','status':'unknown',
                 'guide':{'title':'Pausa','sources':[{'warnings':['Lettura parziale']}],
                          'tracks':{'operations':[{'instructions':['Gioco non avviato']}]}}}
        result=localize_payload(payload)
        self.assertEqual(result['error'],'Partial read')
        for field in ('name','id','status','guide'):self.assertEqual(result[field],payload[field])
        self.assertEqual(payload['error'],'Lettura parziale')

    def test_html_translates_ui_but_not_script_or_style(self):
        text='<html lang="en"><title>Import guide</title><input placeholder="Search"><script>const x="Import guide";</script><style>.Import{}</style></html>'
        italian=localize_html(text,'italian')
        self.assertIn('lang="it"',italian)
        self.assertIn('<title>Importa guida</title>',italian)
        self.assertIn('const x="Import guide";',italian)
        self.assertIn('.Import{}',italian)
        self.assertIn('<title>Import guide</title>',localize_html(italian,'english'))
        sentence="I reviewed the guide&#x27;s instructions and the remaining uncertain source details."
        self.assertIn('Ho rivisto',localize_html('<p>'+sentence+'</p>','italian'))
        self.assertEqual(localize_html('<p>&lt;img&gt; &amp; test</p>'),'<p>&lt;img&gt; &amp; test</p>')

    def test_new_and_legacy_profiles_default_ui_to_english_without_changing_game_language(self):
        with TemporaryDirectory() as folder:
            settings=Path(folder)/'settings.json'
            with patch('hoi4companion.api.SETTINGS',settings),patch('hoi4companion.api.backend_info',return_value={}),\
                 patch('hoi4companion.api.detect_game',return_value=None),patch('hoi4companion.api.RunStore'),\
                 patch('hoi4companion.api.default_guide_id',return_value=None),\
                 patch.object(Store,'_sync_progress'),patch.object(Store,'refresh'):
                fresh=Store();self.assertEqual(fresh.settings['ui_language'],'english')
                settings.write_text(json.dumps({'language':'italian','country':'GER'}),encoding='utf-8')
                legacy=Store();self.assertEqual(legacy.settings['language'],'italian')
                self.assertEqual(legacy.settings['ui_language'],'english')
                self.assertEqual(legacy.settings['country'],'GER')

    def test_http_preference_persists_and_does_not_reindex_or_reset_content_language(self):
        store=Store.__new__(Store);store.lock=RLock()
        store.settings={'language':'italian','country':'GER','ui_language':'english'}
        store.refresh=Mock();store.overview=lambda:{'application':'hoi4companion','settings':store.settings}
        with TemporaryDirectory() as folder,patch('hoi4companion.api.SETTINGS',Path(folder)/'settings.json'):
            with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
                thread=Thread(target=server.serve_forever,daemon=True);thread.start()
                base=f'http://127.0.0.1:{server.server_port}'
                try:
                    with urlopen(base+'/importer') as response:page=response.read().decode('utf-8')
                    self.assertIn('lang="en"',page);self.assertIn('Import guide',page)
                    self.assertLess(page.index('/i18n.js'),page.index('/importer.js'))
                    request=Request(base+'/api/config',data=b'{"ui_language":"italian"}',headers={'Content-Type':'application/json','Origin':base})
                    with urlopen(request) as response:result=json.load(response)
                    self.assertEqual(store.settings['language'],'italian')
                    self.assertEqual(result['settings']['ui_language'],'italian')
                    store.refresh.assert_not_called()
                    self.assertEqual(json.loads((Path(folder)/'settings.json').read_text(encoding='utf-8'))['ui_language'],'italian')
                    with urlopen(base+'/importer') as response:page=response.read().decode('utf-8')
                    self.assertIn('lang="it"',page);self.assertIn('Importa guida',page)
                    with urlopen(base+'/i18n.js') as response:script=response.read().decode('utf-8')
                    self.assertTrue(script.startswith('globalThis.HOI_TRANSLATIONS='))
                    self.assertIn('Interface language',script)
                    invalid=Request(base+'/api/config',data=b'{"ui_language":"unsupported"}',headers={'Content-Type':'application/json','Origin':base})
                    with self.assertRaises(HTTPError):urlopen(invalid)
                    self.assertEqual(store.settings['ui_language'],'italian')
                finally:server.shutdown();thread.join(3)

    def test_all_pages_load_localization_before_ui_scripts(self):
        root=Path(__file__).parents[1]/'src/hoi4companion/web'
        for page in root.glob('*.html'):
            with self.subTest(page=page.name):
                text=page.read_text(encoding='utf-8')
                self.assertIn('<html lang="en">',text)
                self.assertEqual(text.count('src="/i18n.js"'),1)
                self.assertLess(text.index('src="/i18n.js"'),text.index('</head>'))

