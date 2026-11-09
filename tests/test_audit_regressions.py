"""Cross-component audit regressions: lifecycle, concurrent sources and recovery."""
import gc
import json
from pathlib import Path
import struct
import subprocess
from tempfile import TemporaryDirectory
import threading
import unittest
from unittest.mock import patch
import weakref

from hoi4companion.focus_ui_native import UiReader
from hoi4companion.guide_frames import GuideFrames
from test_native_fixture import Fixture

URL='https://www.youtube.com/watch?v=abcdefghijk'
OTHER='https://www.youtube.com/watch?v=lmnopqrstuv'


class AuditRegressions(unittest.TestCase):
    def test_rtti_cache_does_not_retain_closed_readers_and_entire_profiles(self):
        f=Fixture();f.profile.image_base=f.base
        data={0x4000-8:struct.pack('<Q',f.base+0x5000),
              0x5000:struct.pack('<6I',1,0,0,0x6000,0,0x5000),
              0x6010:b'.?AVCButtonStandard@@\0'.ljust(128,b'\0')}
        f.profile.bytes=lambda rva,size:data[rva][:size]
        reader=UiReader(process=f);self.assertEqual(reader._rtti(f.base+0x4000),('.?AVCButtonStandard@@',0))
        ref=weakref.ref(reader);reader.close();del reader;gc.collect()
        self.assertIsNone(ref(), 'A global method cache pins the reader and PE image')

    def test_parallel_source_registration_across_instances_never_loses_a_video(self):
        with TemporaryDirectory() as directory:
            root=Path(directory);source=root/'source.mp4';source.write_bytes(b'video')
            frames=GuideFrames(root/'frames');frames.register(URL,source)
            other=GuideFrames(frames.root)
            first_written=threading.Event();release=threading.Event();second_done=threading.Event();errors=[]
            original=Path.write_text
            def write(path,*args,**kwargs):
                value=original(path,*args,**kwargs)
                if path.parent==frames.root and path.suffix=='.tmp' and threading.current_thread().name=='source-first':
                    first_written.set();release.wait(2)
                return value
            def run(instance,url):
                try:instance.register(url,source)
                except Exception as error:errors.append(error)
                finally:
                    if threading.current_thread().name=='source-second':second_done.set()
            with patch.object(Path,'write_text',write):
                first=threading.Thread(target=run,args=(frames,URL),name='source-first')
                second=threading.Thread(target=run,args=(other,OTHER),name='source-second')
                first.start();self.assertTrue(first_written.wait(1));second.start()
                second_done.wait(.15);release.set();first.join(2);second.join(2)
            self.assertFalse(first.is_alive() or second.is_alive());self.assertEqual(errors,[])
            self.assertEqual(set(json.loads((frames.root/'sources.json').read_text())),{URL,OTHER})

    def test_failed_frame_extraction_never_becomes_a_successful_cached_frame(self):
        from PIL import Image
        with TemporaryDirectory() as directory:
            root=Path(directory);source=root/'source.mp4';source.write_bytes(b'video')
            frames=GuideFrames(root/'frames');frames.register(URL,source);calls=[]
            def render(args,**kwargs):
                calls.append(args);target=Path(args[-1])
                if len(calls)==1:
                    target.write_bytes(b'partial-jpeg');raise ValueError('Encoder failed')
                Image.new('RGB',(8,8),'red').save(target,'JPEG')
            with patch('hoi4companion.guide_frames.ffmpeg_path',return_value='ffmpeg'),patch('hoi4companion.guide_frames.duration',return_value=10),patch('hoi4companion.guide_frames.command',side_effect=render):
                with self.assertRaises(ValueError):frames.frame(URL,2)
                target=frames.frame(URL,2)
                with Image.open(target) as image:image.verify()
                self.assertEqual(frames.frame(URL,2),target);self.assertEqual(len(calls),2)
            self.assertEqual(len(list(frames.root.glob('*.jpg'))),1)

    def test_frame_timeout_is_a_reported_error_and_leaves_no_partial_frame(self):
        with TemporaryDirectory() as directory:
            source=Path(directory)/'source.mp4';source.write_bytes(b'video')
            frames=GuideFrames(Path(directory)/'frames');frames.register(URL,source)
            def render(args,**kwargs):
                Path(args[-1]).write_bytes(b'partial');raise subprocess.TimeoutExpired(args,40)
            with patch('hoi4companion.guide_frames.ffmpeg_path',return_value='ffmpeg'),patch('hoi4companion.guide_frames.duration',return_value=10),patch('hoi4companion.guide_frames.command',side_effect=render):
                with self.assertRaisesRegex(ValueError,'tempo massimo'):frames.frame(URL,2)
            self.assertEqual(list(frames.root.glob('*.jpg')),[])
