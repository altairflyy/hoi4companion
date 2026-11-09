"""Native passive source intake preserves text identity and forbids private targets."""
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.import_sources import acquire_source,parse_html,parse_captions

class NativeSourceTests(unittest.TestCase):
    def test_manual_sources_have_identical_hashes_types_text_and_warnings(self):
        for url in ('','https://youtu.be/abcdefghijk','https://example.com/article'):
            for text in ('Research tools.','  preserve spaces  ','Équipement — Łódź\nNext step'):
                self.assertEqual(_request(0,'import_source',url=url,text=text),acquire_source(url=url,text=text))
        for text in ('','x'*180001):
            with self.assertRaises(ValueError):_request(0,'import_source',text=text)

    def test_html_captions_are_passive_and_match_supported_text_sources(self):
        for html in ('<main><h1>Guide</h1><script>evil()</script><p>Build tools</p></main>','<p>Text &amp; stuff</p><div>Next <b>step</b></div>','plain text','<nav>hidden</nav><article>Visible<br>new line</article>','<style>.bad {}</style><p>Été</p>'):
            self.assertEqual(_request(0,'source_html',text=html),parse_html(html))
        captions={'events':[{'tStartMs':5000,'segs':[{'utf8':'Research '},{'utf8':'tools'}]},{'tStartMs':-1,'segs':[{'utf8':'bad'}]},{'segs':[]}]}
        self.assertEqual(_request(0,'source_captions',value=captions),parse_captions(captions))

    def test_private_reserved_and_mapped_addresses_never_reach_tools(self):
        for ip in ('127.0.0.1','10.0.0.2','192.168.1.1','172.16.2.2','169.254.169.254','100.64.0.1','0.0.0.0','192.0.2.1','198.51.100.1','203.0.113.1','::1','::','fc00::1','fe80::1','::ffff:192.168.1.1','2001:db8::1'):
            self.assertFalse(_request(0,'source_global_ip',ip=ip),ip)
        for ip in ('1.1.1.1','8.8.8.8','2606:4700:4700::1111'):
            self.assertTrue(_request(0,'source_global_ip',ip=ip),ip)
        for url in ('http://127.0.0.1/','http://[::1]/','file:///etc/passwd','https://u:p@example.com'):
            with self.assertRaises(ValueError):_request(0,'import_source',url=url)

    def test_provider_draft_uses_supplied_sources_and_never_installs(self):
        import json
        sources=[acquire_source(text='Research tools.')]
        packet={'instructions':'ignored transport fixture','sources':sources,'catalog':[]}
        body=_request(0,'import_provider_body',packet=packet,model='test-model')
        self.assertFalse(body['store']);self.assertEqual(body['max_output_tokens'],16000)
        self.assertEqual(json.loads(body['input']),{'sources':sources,'catalog':[]})
        self.assertEqual(body['text']['format'],{'type':'json_object'})
        self.assertNotIn('api_key',body)
        authoring={'format':'hoi4-guide-authoring','authoring_schema_version':1,'sources':[{'id':'invented','text':'altered'}],
            'guide':{'id':'provider-draft','title':'Fixture','steps':[{'type':'research','target':'Tools','evidence':[{'source_id':sources[0]['id'],'basis':'explicit','quote':sources[0]['text']}]}]}}
        response={'status':'completed','output':[{'type':'message','content':[{'type':'output_text','text':json.dumps(authoring)}]}]}
        draft=_request(0,'import_provider_document',response=response,sources=sources)
        self.assertEqual(draft['sources'],sources);self.assertEqual(draft['guide']['id'],'provider-draft')
        for response in ({'status':'incomplete'},{'status':'completed','output':[]},{'status':'completed','output':[{'type':'message','content':[{'type':'refusal','refusal':'No'}]}]}):
            with self.assertRaises(ValueError):_request(0,'import_provider_document',response=response,sources=sources)
