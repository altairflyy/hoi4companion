from email.message import Message
from io import BytesIO
from http.server import ThreadingHTTPServer
from threading import Thread
from urllib.error import HTTPError
from urllib.request import Request,urlopen
import unittest

from hoi4companion.api import make_handler


class HttpRejectionTests(unittest.TestCase):
    def request(self,body=b'{}',declared='2',transfer=None):
        cls=make_handler(object());handler=cls.__new__(cls)
        handler.path='/api/guide/progress';handler.headers=Message()
        for key,value in {'Origin':'https://example.invalid','Host':'127.0.0.1:8766','Content-Length':declared}.items():handler.headers[key]=value
        if transfer:handler.headers['Transfer-Encoding']=transfer
        handler.rfile=BytesIO(body)
        class Connection:
            timeout=None
            def gettimeout(self):return self.timeout
            def settimeout(self,value):self.timeout=value
        handler.connection=Connection();responses=[];handler.json=lambda data,status:responses.append((data,status))
        handler.do_POST()
        return handler,responses

    def test_foreign_origin_is_rejected_and_small_body_drained_before_close(self):
        handler,responses=self.request()
        self.assertEqual(responses,[({'error':'Origine non autorizzata'},403)])
        self.assertEqual(handler.rfile.tell(),2)
        self.assertIsNone(handler.connection.timeout)

    def test_rejection_never_drain_unbounded_or_ambiguous_uploads(self):
        for declared,transfer in [('200000000',None),('invalid',None),('-1',None),('2','chunked')]:
            with self.subTest(declared=declared,transfer=transfer):
                handler,responses=self.request(declared=declared,transfer=transfer)
                self.assertEqual(handler.rfile.tell(),0);self.assertEqual(responses[0][1],403)

    def test_repeated_real_requests_receive_403_without_connection_resets(self):
        handler=make_handler(object());handler.log_message=lambda *args:None
        with ThreadingHTTPServer(('127.0.0.1',0),handler) as server:
            thread=Thread(target=server.serve_forever,daemon=True);thread.start()
            try:
                for number in range(50):
                    with self.subTest(request=number),self.assertRaises(HTTPError) as rejected:
                        urlopen(Request(f'http://127.0.0.1:{server.server_port}/api/guide/progress',data=b'{}',
                            headers={'Origin':'https://example.invalid','Content-Type':'application/json'}),timeout=2)
                    self.assertEqual(rejected.exception.code,403)
                    rejected.exception.close()
            finally:server.shutdown();thread.join(timeout=2)
