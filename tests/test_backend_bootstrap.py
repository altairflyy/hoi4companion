"""Native startup must preserve user data and avoid developer-specific paths."""
import json,os,subprocess,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
class NativeBootstrapTests(unittest.TestCase):
    def config(self,directory,**settings):
        folder=Path(directory);path=folder/'settings.json';path.write_text(json.dumps(settings),encoding='utf8');before=path.read_bytes()
        executable=os.environ.get('HOI4_BACKEND_EXE',str(ROOT/'.local/rust-backend-target/debug/hoi4-backend.exe'))
        completed=subprocess.run([executable,'--resources',str(ROOT/'src/hoi4companion'),'--data-dir',str(folder),'--print-config','--no-discovery'],capture_output=True,text=True,encoding='utf8',timeout=10)
        self.assertEqual(completed.returncode,0,completed.stderr);self.assertEqual(path.read_bytes(),before)
        return json.loads(completed.stdout)
    def test_defaults_are_english_and_data_paths_are_local_to_selected_root(self):
        with tempfile.TemporaryDirectory() as directory:
            config=self.config(directory)
            self.assertEqual(config['catalog']['settings']['ui_language'],'english')
            self.assertEqual(config['catalog']['settings']['mods'],'auto')
            self.assertEqual(Path(config['path']),Path(directory)/'guide-progress.sqlite3')
            self.assertEqual(Path(config['selection_path']),Path(directory)/'active-guide.json')
            self.assertEqual(Path(config['catalog']['settings_path']),Path(directory)/'settings.json')
            self.assertNotIn('provider',config)
            self.assertNotIn('valer',config['catalog']['settings']['game_dir'])
    def test_persisted_installation_language_country_and_selection_are_not_replaced(self):
        with tempfile.TemporaryDirectory() as directory:
            folder=Path(directory);game=folder/'custom-game';game.mkdir();(game/'hoi4.exe').write_bytes(b'fixture')
            active=folder/'active-guide.json';active.write_text('{"guide_id":"custom"}',encoding='utf8')
            config=self.config(directory,game_dir=str(game),docs_dir=str(folder/'custom-documents'),country='HUN',ui_language='italian',language='italian',mods='off',tree_id='hungarian_focus',snapshot_path='')
            self.assertEqual(config['catalog']['settings']['country'],'HUN')
            self.assertEqual(config['catalog']['settings']['ui_language'],'italian')
            self.assertEqual(config['provider']['path'],str(game/'hoi4.exe'))
            self.assertEqual(active.read_text(),'{"guide_id":"custom"}')
            self.assertEqual(len(config['provider']['manifest_sha256']),64)
