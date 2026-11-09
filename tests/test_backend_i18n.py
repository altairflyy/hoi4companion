"""Native UI localization never rewrites guide content or entity identifiers."""
from pathlib import Path
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.i18n import localize_html,localize_payload,translate,catalog

class BackendI18nTests(unittest.TestCase):
    def test_diagnostics_templates_preserve_arguments_and_content_boundaries(self):
        for locale,section in [('english','legacy'),('italian','messages')]:
            for text in catalog()[section]:
                message=text
                for i in range(8):message=message.replace('{'+str(i)+'}',f'argument_{i} <GER_1> {{nested}}')
                self.assertEqual(_request(0,'ui_translate',text=message,locale=locale),translate(message,locale),text)
        value={'error':'Lettura parziale','name':'Pausa','guide':{'tracks':{'operations':[{'instructions':['Gioco non avviato']}]},'warnings':['Lettura parziale']},'items':[{'reason':'Gioco non avviato'}]}
        self.assertEqual(_request(0,'ui_payload',value=value,locale='english'),localize_payload(value))

    def test_html_preserves_scripts_styles_escaping_and_translates_attributes(self):
        text='<html lang="en"><title>Import guide</title><input placeholder="Search"><script>const x="Import guide";</script><style>.Import{}</style><p>I reviewed the guide&#x27;s instructions and the remaining uncertain source details.</p><p>&lt;img&gt; &amp; test</p></html>'
        for locale in ('english','italian'):
            self.assertEqual(_request(0,'ui_html',text=text,locale=locale),localize_html(text,locale))

    def test_all_packaged_pages_preserve_public_localization_output(self):
        import hoi4companion
        root=Path(hoi4companion.__file__).with_name('web')
        for page in root.glob('*.html'):
            text=page.read_text(encoding='utf8')
            for locale in ('english','italian'):
                with self.subTest(page=page.name,locale=locale):
                    self.assertEqual(_request(0,'ui_html',text=text,locale=locale),localize_html(text,locale))
