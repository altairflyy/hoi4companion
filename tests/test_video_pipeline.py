import json
import sys
import threading
import time
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from hoi4companion.guide_media import VideoJobs


class VideoPipelineTests(unittest.TestCase):
    def test_retry_does_not_report_progress_from_the_previous_encode(self):
        with TemporaryDirectory() as d:
            root=Path(d);(root/'encoding-progress.txt').write_text('out_time_us=999999000\n')
            jobs=VideoJobs(root/'jobs');key='a'*32;jobs.jobs[key]={'status':'running'};reported=[]
            class Process:
                returncode=0
                def __init__(self):self.states=iter([None,0])
                def poll(self):return next(self.states,0)
            with patch('hoi4companion.guide_media.subprocess.Popen',return_value=Process()),patch('hoi4companion.guide_media.time.sleep'),patch.object(jobs,'update',side_effect=lambda key,**kw:reported.append(kw)):
                jobs._encode(key,'ffmpeg',root/'source.mp4',root/'master.mp4',[])
            self.assertFalse(any(row.get('encoded_seconds',0)>1 for row in reported))

    def test_completed_cache_checks_hash_and_explicit_source_refresh_bypasses_it(self):
        from hoi4companion.guide_frames import GuideFrames
        from hoi4companion.guide_media import PRESET_VERSION,ENCODING_PROFILE
        from hoi4companion.video_cache import digest
        with TemporaryDirectory() as d:
            root=Path(d);source=root/'source.mp4';source.write_bytes(b'source');url='https://www.youtube.com/watch?v=abcdefghijk'
            GuideFrames(root/'guide-frames').register(url,source)
            jobs=VideoJobs(root/'guide-videos');key='a'*32;folder=jobs.root/key;folder.mkdir(parents=True);part=folder/'parte-01.mp4';part.write_bytes(b'part')
            manifest={'preset_version':PRESET_VERSION,'encoding_profile':ENCODING_PROFILE,'source_url':url,'fps':15,
                'source_sha256':digest(source),'max_part_bytes':150000000,'overlap_seconds':10,'segment_seconds':300,
                'parts':[{'name':part.name,'bytes':4,'sha256':digest(part)}]}
            (folder/'manifest.json').write_text(json.dumps(manifest))
            self.assertEqual(jobs.start(url)['id'],key)
            part.write_bytes(b'nope')
            with patch('hoi4companion.guide_media.threading.Thread') as worker:
                self.assertEqual(jobs.start(url)['status'],'running');worker.return_value.start.assert_called_once()
            jobs.jobs.clear();part.write_bytes(b'part')
            with patch('hoi4companion.guide_media.threading.Thread'):
                job=jobs.start(url,refresh_source=True);self.assertTrue(job['refresh_source']);self.assertNotEqual(job['id'],key)

    def test_rechunking_reuses_master_without_encoder_or_download(self):
        from hoi4companion.guide_frames import GuideFrames
        with TemporaryDirectory() as d:
            root=Path(d);source=root/'source.mp4';source.write_bytes(b'source')
            url='https://www.youtube.com/watch?v=abcdefghijk';frames=GuideFrames(root/'guide-frames');frames.register(url,source,{'duration':40})
            jobs=VideoJobs(root/'guide-videos');jobs.root.mkdir();key,identity=jobs.cache.identity(source)
            encoded=root/'encoded.mp4';encoded.write_bytes(b'master')
            jobs.cache.save(key,identity,encoded,{'source_url':url,'duration_seconds':40,'encoder':'CPU x265','quality_mode':'CRF 22'})
            def split(ff,master,folder,*args,**kw):
                (folder/'parte-01.mp4').write_bytes(b'part')
                return [{'name':'parte-01.mp4','start_seconds':0,'end_seconds':40,'bytes':4}]
            with patch('hoi4companion.guide_media.media_info',return_value={'duration_seconds':40,'width':1280,'height':720}),patch('hoi4companion.guide_media.split_video',side_effect=split),patch.object(jobs,'_encode',side_effect=AssertionError('Unexpected re-encode')),patch.object(jobs,'_download',side_effect=AssertionError('Unexpected download')):
                for job,seconds in [('a'*32,300),('b'*32,200)]:
                    jobs.jobs[job]={'status':'running','parts':[]}
                    with patch('hoi4companion.guide_media.SEGMENT_SECONDS',seconds):jobs._run(job,url)
                    self.assertEqual(jobs.status(job)['status'],'completed');self.assertTrue(jobs.status(job)['master_reused'])

    def test_master_cache_is_independent_of_chunk_profile_and_checks_content(self):
        from hoi4companion.video_cache import MasterCache
        with TemporaryDirectory() as d:
            root=Path(d);source=root/'source.mp4';source.write_bytes(b'source')
            cache=MasterCache(root/'masters',{'fps':15,'quality':22})
            key,identity=cache.identity(source);encoded=root/'encoded.mp4';encoded.write_bytes(b'encoded')
            cache.save(key,identity,encoded,{'source_url':'https://youtu.be/abcdefghijk','duration_seconds':40})
            other=MasterCache(root/'masters',{'fps':15,'quality':22})
            self.assertEqual(other.identity(source)[0],key);self.assertIsNotNone(other.load(key))
            self.assertNotEqual(MasterCache(root/'masters',{'fps':15,'quality':24}).identity(source)[0],key)
            source.write_bytes(b'new source');self.assertNotEqual(other.identity(source)[0],key)
            cache.path(key).write_bytes(b'corrupt');self.assertIsNone(other.load(key))

    def test_close_stops_an_owned_probe_subprocess(self):
        with TemporaryDirectory() as d:
            jobs=VideoJobs(d);key='a'*32;jobs.jobs[key]={'status':'running'};errors=[]
            def probe():
                try:jobs._command(key,[sys.executable,'-c','import time;time.sleep(60)'],timeout=70)
                except ValueError as exc:errors.append(str(exc))
            worker=threading.Thread(target=probe);worker.start()
            deadline=time.monotonic()+5
            while key not in jobs.processes and worker.is_alive() and time.monotonic()<deadline:time.sleep(.01)
            self.assertIn(key,jobs.processes);process=jobs.processes[key]
            jobs.close();worker.join(timeout=8)
            self.assertFalse(worker.is_alive());self.assertIsNotNone(process.poll());self.assertTrue(errors)
            self.assertNotIn(key,jobs.processes)

    def test_cleanup_keeps_current_outputs_registered_sources_and_unmanaged_files(self):
        with TemporaryDirectory() as d:
            root=Path(d);jobs=VideoJobs(root/'guide-videos');jobs.root.mkdir()
            url='https://www.youtube.com/watch?v=abcdefghijk';old=jobs.root/('a'*32);old.mkdir()
            current=jobs.root/('b'*32);current.mkdir()
            for folder in (old,current):
                (folder/'manifest.json').write_text(json.dumps({'source_url':url}))
                (folder/'parte-01.mp4').write_bytes(b'video')
            (old/'notes.txt').write_text('keep');source=old/'source.mp4';source.write_bytes(b'original')
            jobs.cleanup(url,current.name,protected=[source])
            self.assertTrue(source.exists());self.assertTrue((old/'notes.txt').exists())
            self.assertFalse((old/'parte-01.mp4').exists());self.assertTrue((current/'parte-01.mp4').exists())


if __name__=='__main__':unittest.main()
