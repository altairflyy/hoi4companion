import os,sys,unittest
from pathlib import Path
from unittest.mock import patch
from hoi4companion.import_sources import ytdlp_command
from hoi4companion.paths import javascript_runtime


class YoutubeToolTests(unittest.TestCase):
    def test_source_module_works_without_a_downloader_on_path(self):
        with patch.dict(os.environ,{},clear=True),patch.object(sys,'frozen',False,create=True),patch('hoi4companion.import_sources.importlib.util.find_spec',return_value=object()),patch('hoi4companion.import_sources.ytdlp_path',return_value=None):
            self.assertEqual(ytdlp_command(),[sys.executable,'-m','yt_dlp'])

    def test_explicit_downloader_override_and_standalone_fallback_are_preserved(self):
        with patch.dict(os.environ,{'HOI4_YTDLP':'C:/Tools/my downloader.exe'},clear=True),patch('hoi4companion.import_sources.importlib.util.find_spec') as spec:
            self.assertEqual(ytdlp_command(),['C:/Tools/my downloader.exe']);spec.assert_not_called()
        with patch.dict(os.environ,{},clear=True),patch.object(sys,'frozen',False,create=True),patch('hoi4companion.import_sources.importlib.util.find_spec',return_value=None),patch('hoi4companion.import_sources.ytdlp_path',return_value='C:/Tools/yt-dlp.exe'):
            self.assertEqual(ytdlp_command(),['C:/Tools/yt-dlp.exe'])

    def test_source_node_is_selected_and_portable_build_never_borrows_system_node(self):
        with patch.object(sys,'frozen',False,create=True),patch('hoi4companion.paths.shutil.which',return_value='C:/Tools/node.exe'):
            self.assertEqual(javascript_runtime(),Path('C:/Tools/node.exe'))
        with patch.object(sys,'frozen',True,create=True),patch('hoi4companion.paths.bundled_node',return_value=Path('C:/Bundle/node.exe')),patch('hoi4companion.paths.shutil.which') as which:
            self.assertEqual(javascript_runtime(),Path('C:/Bundle/node.exe'));which.assert_not_called()
        with patch.object(sys,'frozen',True,create=True),patch('hoi4companion.paths.bundled_node',return_value=None),patch('hoi4companion.paths.shutil.which') as which:
            self.assertIsNone(javascript_runtime());which.assert_not_called()
