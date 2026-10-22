import json
from pathlib import Path
from tempfile import TemporaryDirectory
from threading import Thread,RLock
import unittest
from unittest.mock import patch
from http.server import ThreadingHTTPServer
from urllib.request import Request,urlopen
from urllib.error import HTTPError
from hoi4companion.api import Store,make_handler
from hoi4companion.guide import normalise_guide
from hoi4companion.guide_importer import Importer


class AssociationApiTests(unittest.TestCase):
    def test_local_correction_and_restore_preserve_run_and_reject_foreign_posts(self):
        with TemporaryDirectory() as directory:
            root=Path(directory);local=root/'guides';local.mkdir();source=normalise_guide({'schema_version':1,'id':'test-association','tracks':{'research':[{'key':'r','action':{'kind':'research','entity':{'kind':'technology','id':'one'}}}]}})
            path=local/'guide.json';path.write_text(json.dumps(source),encoding='utf-8');original=path.read_bytes()
            catalog=[{'kind':'technology','id':key,'name':key} for key in ('one','two')]
            store=Store.__new__(Store);store.lock=RLock();store.guide_revision=0;store._importer=Importer(lambda:catalog)
            store.trees={};store.labels={};store.demo=False;store.settings={'country':'TST'};store.technologies={'technologies':{},'folders':{}}
            with patch('hoi4companion.guide.LOCAL_GUIDES',local),patch('hoi4companion.guide_associations.DIRECTORY',root/'associations'):
                with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
                    thread=Thread(target=server.serve_forever,daemon=True);thread.start();base=f'http://127.0.0.1:{server.server_port}'
                    def call(url,body=None,origin=None):
                        headers={'Content-Type':'application/json'}
                        if origin:headers['Origin']=origin
                        with urlopen(Request(base+url,data=json.dumps(body).encode() if body is not None else None,headers=headers),timeout=3) as response:return json.load(response)
                    try:
                        guide_id=source['id'];saved=call('/api/guide/progress?id='+guide_id);before=call('/api/guide/associations?id='+guide_id)
                        old={'kind':'technology','id':'one'};new={'kind':'technology','id':'two'}
                        update={'guide_id':guide_id,'key':'r','expected':old,'entity':new}
                        with self.assertRaises(HTTPError) as error:call('/api/guide/associations',update,'https://unrelated.test')
                        self.assertEqual(error.exception.code,403)
                        after=call('/api/guide/associations',update);self.assertEqual(after['guide']['tracks']['research'][0]['target'],'two')
                        self.assertEqual(call('/api/guide?id='+guide_id)['tracks']['research'][0]['target'],'two')
                        self.assertEqual(call('/api/guide/progress?id='+guide_id)['run_id'],saved['run_id']);self.assertEqual(path.read_bytes(),original)
                        with self.assertRaises(HTTPError):call('/api/guide/associations',update)
                        restored=call('/api/guide/associations',{**update,'expected':new,'entity':None});self.assertEqual(restored['changes'],[])
                        with urlopen(base+'/associations') as response:self.assertIn(b'associations.js',response.read())
                    finally:server.shutdown();thread.join(timeout=3)
