"""Actual native launch/reuse/stop in isolated application data, no game UI."""
import http.client,json,os,socket,subprocess,tempfile,time,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]

class NativeDesktopTests(unittest.TestCase):
    def test_launcher_reuses_service_and_stops_only_with_its_data_capability(self):
        desktop=Path(os.environ.get('HOI4_DESKTOP_EXE',ROOT/'.local/rust-backend-target/debug/hoi4-desktop.exe'))
        backend=Path(os.environ.get('HOI4_BACKEND_EXE',ROOT/'.local/rust-backend-target/debug/hoi4-backend.exe'))
        with socket.socket() as port_socket:
            port_socket.bind(('127.0.0.1',0));port=port_socket.getsockname()[1]
        with tempfile.TemporaryDirectory() as directory:
            data=Path(directory);(data/'settings.json').write_text('{"game_dir":"", "country":"AUS", "ui_language":"italian"}',encoding='utf8')
            # No discovery is needed when the user deliberately selected an
            # existing custom location that lacks game data (demo mode).
            (data/'settings.json').write_text(json.dumps({'game_dir':str(data/'unconfigured-game'),'country':'AUS','ui_language':'italian'}),encoding='utf8')
            env={**os.environ,'HOI4_BACKEND_EXE':str(backend)}
            command=[str(desktop),'--port',str(port),'--resources',str(ROOT/'src/hoi4companion'),'--data-dir',directory,'--no-browser','--no-hud']
            def run(*extra):return subprocess.run([*command,*extra],env=env,capture_output=True,text=True,encoding='utf8',timeout=30,creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
            def api(path,body=None):
                connection=http.client.HTTPConnection('127.0.0.1',port,timeout=5)
                try:
                    connection.request('POST' if body is not None else 'GET',path,json.dumps(body) if body is not None else None,{'Content-Type':'application/json'})
                    reply=connection.getresponse();return reply.status,json.loads(reply.read())
                finally:connection.close()
            launched=False
            try:
                first=run();self.assertEqual(first.returncode,0,first.stderr);launched=True
                descriptor=api('/api/backend')[1];self.assertEqual(descriptor['resident_python'],False)
                ready=json.loads((data/f'native-service-{port}.json').read_text());pid=ready['pid']
                self.assertEqual(api('/api/config')[1]['settings']['ui_language'],'italian')
                self.assertEqual(run().returncode,0)
                self.assertEqual(json.loads((data/f'native-service-{port}.json').read_text())['pid'],pid)
                self.assertEqual(api('/api/shutdown',{'token':'0'*64})[0],400)
                self.assertEqual(api('/api/backend')[0],200)
                stopped=run('--stop');self.assertEqual(stopped.returncode,0,stopped.stderr)
                self.assertEqual(json.loads(stopped.stdout)['pid'],pid)
                deadline=time.monotonic()+10
                while time.monotonic()<deadline:
                    try:api('/api/backend')
                    except OSError:break
                    time.sleep(.05)
                else:self.fail('Owned native service did not stop')
                launched=False
            finally:
                if launched:run('--stop')
