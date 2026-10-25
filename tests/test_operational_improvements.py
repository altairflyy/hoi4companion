import copy
import json
import struct
import subprocess
import threading
from http.server import ThreadingHTTPServer
from types import SimpleNamespace
from urllib.request import Request,urlopen
from urllib.error import HTTPError
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

from hoi4companion.contracts import normalise_state
from hoi4companion.guide import normalise_guide
from hoi4companion.guide_runtime import evaluate
from hoi4companion.operation_import import bind_operations
from hoi4companion.operation_import import operation_catalog
from hoi4companion.api import make_handler
from hoi4companion.country import CountryReader
from hoi4companion.live import LiveReadError
from hoi4companion.guide_frames import frame_reference, GuideFrames
from hoi4companion.guide_media import VideoJobs, split_video, choose_encoder, PRESET_VERSION, ENCODING_PROFILE
from test_native_fixture import Fixture


def definition(params):
    return {'id':'generic','tracks':{'operations':[{'key':'order','action':{'kind':'manual','params':params}}]}}


class ProjectFixture(Fixture):
    def __init__(self):
        super().__init__()
        self.status=self.object('CProgramStatus@NProject')
        self.pool=self.object('CProjectPool@NProject')
        self.program=self.object('CProgram@NProject')
        self.project=self.object('CProject@NProject')
        self.template=self.object('CProjectTemplate@NProject')
        self.machine=self.object('CProjectStateMachine@NProject')
        self.final=self.object('CSimpleProjectState@NProject')
        self.put(self.country+0xFA8,'<Q',self.status)
        self.put(self.status+0x10,'<i',1)
        self.put(self.status+0x18,'<Q',self.pool)
        self.put(self.pool+8,'<i',1)
        self.array(self.status+0x20,[self.program])
        self.put(self.program+0x18,'<i',1)
        self.put(self.program+8,'<II',83,7)
        self.put(self.program+0x50,'<II',123,1)
        self.string(self.program+0x20,'land_facility')
        self.put(self.pool+0x10,'<Qii',self.project,1,1)
        self.put(self.project+0x20,'<Q',self.template+0x18)
        self.put(self.project+0x18,'<II',123,1)
        self.string(self.template+0x238,'project_one')
        self.put(self.project+0x28,'<Q',self.machine)
        self.put(self.machine+8,'<Q',self.final)
        self.put(self.machine+0x10,'<B',1)
        self.put(self.project+0x60,'<II',83,7)

    def read_projects(self):
        reader=CountryReader(self,self.world,self.country,1)
        result=reader.projects();reader.s.verify();return result


class OperationalImprovementsTests(unittest.TestCase):
    def test_catalog_includes_declared_duplicate_roles_and_national_project_names(self):
        with TemporaryDirectory() as d:
            root=Path(d);equipment=root/'common/units/equipment';equipment.mkdir(parents=True)
            (equipment/'a.txt').write_text('equipments={ base={is_archetype=yes} base_1={archetype=base} } duplicate_archetypes={ role={archetype=base variant_name={role_1=role_name}} }')
            projects=root/'common/special_projects/projects';projects.mkdir(parents=True)
            (projects/'project.txt').write_text('project_one={ }')
            rows,_=operation_catalog(root,[],{}, {'role_name':'Role equipment','project_one':'Generic project','TST_project_one':'National project'})
            self.assertEqual(next(r for r in rows if r['id']=='role_1')['family_id'],'role')
            self.assertIn('Role equipment',next(r for r in rows if r['id']=='role_1')['aliases'])
            g=definition({'category':'projects','project_name':'National project'})
            bind_operations(g,rows)
            self.assertEqual(g['tracks']['operations'][0]['action']['params']['project_id'],'project_one')
            self.assertEqual(g['tracks']['operations'][0]['action']['kind'],'special_project')

    def test_special_project_is_parallel_to_normal_research_instead_of_an_implicit_manual_gate(self):
        g=definition({'category':'projects','project_name':'Project'})
        g['tracks']={'research':g['tracks'].pop('operations')+[{'key':'tech','action':{'kind':'research','entity':{'kind':'technology','id':'technology'}}}]}
        bind_operations(g,[{'kind':'project','id':'p','name':'Project'}])
        observed=lambda value:{'value':value,'status':'observed','source':'fixture'}
        state={'schema_version':1,'usable':True,'facts':{k:observed(v) for k,v in {
            'projects.active':['p'],'projects.completed':[],'research.active':[],'research.completed':[],
            'research.slots':[{'technology_id':None}]}.items()}}
        result=evaluate(g,state)
        self.assertEqual(result['steps'][0]['status'],'active')
        self.assertEqual(result['parallel']['research']['steps'],['tech'])

    def test_video_http_resume_download_origin_guard_and_source_frame(self):
        with TemporaryDirectory() as d:
            root=Path(d);key='a'*32;folder=root/key;folder.mkdir();part=folder/'parte-01.mp4';part.write_bytes(b'video')
            manifest={'preset_version':PRESET_VERSION,'encoding_profile':ENCODING_PROFILE,'audio_status':'absent','source_url':'https://www.youtube.com/watch?v=Ht4e89B02Wo','fps':15,'max_part_bytes':150000000,'overlap_seconds':10,'segment_seconds':300,'parts':[{'name':part.name,'bytes':5,'start_seconds':0,'end_seconds':30,'url':f'/api/import/video/file?id={key}&name={part.name}'}]}
            (folder/'manifest.json').write_text(json.dumps(manifest));(folder/'descrizione.txt').write_text('description')
            jobs=VideoJobs(root);frames=GuideFrames(root/'frames');image=root/'image.jpg';image.write_bytes(b'jpeg')
            guide=definition({'source_seconds':10});guide['sources']=[{'id':'s','type':'youtube','url':manifest['source_url']}]
            store=SimpleNamespace(video_jobs=jobs,guide_frames=frames,walkthrough=lambda id:guide)
            with ThreadingHTTPServer(('127.0.0.1',0),make_handler(store)) as server:
                thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start();base=f'http://127.0.0.1:{server.server_port}'
                try:
                    request=Request(base+'/api/import/video',data=json.dumps({'url':manifest['source_url']}).encode(),headers={'Content-Type':'application/json'})
                    with urlopen(request) as response:self.assertEqual(json.load(response)['status'],'completed')
                    refreshed=Request(base+'/api/import/video',data=json.dumps({'url':manifest['source_url'],'refresh_source':True}).encode(),headers={'Content-Type':'application/json'})
                    with patch.object(jobs,'start',return_value=jobs.status(key)) as start:
                        with urlopen(refreshed) as response:self.assertEqual(json.load(response)['status'],'completed')
                        start.assert_called_once_with(manifest['source_url'],refresh_source=True)
                    invalid=Request(base+'/api/import/video',data=json.dumps({'url':manifest['source_url'],'refresh_source':'yes'}).encode(),headers={'Content-Type':'application/json'})
                    with self.assertRaises(HTTPError) as error:urlopen(invalid)
                    self.assertEqual(error.exception.code,400)
                    with urlopen(base+f'/api/import/video/file?id={key}&name={part.name}') as response:self.assertEqual(response.read(),b'video')
                    request.add_header('Origin','https://untrusted.example')
                    with self.assertRaises(HTTPError) as error:urlopen(request)
                    self.assertEqual(error.exception.code,403)
                    with patch.object(frames,'frame',return_value=image):
                        with urlopen(base+'/api/guide/frame?id=generic&key=order') as response:self.assertEqual(response.read(),b'jpeg')
                finally:server.shutdown();thread.join(timeout=3)

    def test_native_project_start_completion_and_cross_country_guards(self):
        f=ProjectFixture();row=f.read_projects()['items'][0]
        self.assertEqual((row['active'],row['completed']),(True,False))
        # Final state requires the exact state-machine marker and final label.
        final=f.machine+0xE8
        f.put(final,'<Q',struct.unpack('<Q',f.read(f.final,8))[0])
        f.put(f.machine+8,'<Q',final);f.put(f.machine+0x10,'<B',0)
        f.string(final+0x58,'RESEARCH_COMPLETED')
        row=f.read_projects()['items'][0]
        self.assertEqual((row['active'],row['completed']),(False,True))
        f.put(f.program+0x50,'<II',999,1)
        self.assertTrue(f.read_projects()['items'][0]['completed'])
        for attr,offset in [('status',0x10),('pool',8),('program',0x18)]:
            other=ProjectFixture();other.put(getattr(other,attr)+offset,'<i',2)
            with self.assertRaises(LiveReadError):other.read_projects()
        f=ProjectFixture();f.put(f.program+0x50,'<II',999,1)
        with self.assertRaises(LiveReadError):f.read_projects()

    def test_projects_are_unknown_when_missing_wrong_country_or_partial(self):
        sample={'country':'TST','country_overview':{'country':'TST','projects':{'status':'observed','items':[{'id':'p','active':True,'completed':False}]}}}
        state={'usable':True,'sample':sample}
        self.assertEqual(normalise_state(state)['facts']['projects.active']['value'],['p'])
        for mutate in (lambda s:s['country_overview'].update(country='OTH'),lambda s:s['country_overview']['projects']['items'][0].pop('completed')):
            changed=copy.deepcopy(sample);mutate(changed)
            self.assertEqual(normalise_state({'usable':True,'sample':changed})['facts']['projects.active']['status'],'unknown')

    def test_requested_production_counts_pending_factories_and_layout(self):
        catalog=[{'kind':'equipment','id':'rifle','name':'Rifles'}]
        g=definition({'category':'production','production_targets':[{'equipment_id':'rifle','requested_factories':10}]})
        bind_operations(g,catalog)
        state={'usable':True,'sample':{'country':'TST','country_overview':{'country':'TST','production':{'status':'observed','lines':[
            {'id':'1','equipment':'rifle','name':'Rifle','category':'military','requested_factories':10,'assigned_factories':3,'active_factories':3,'damaged_factories':0}]}}}}
        row=evaluate(g,state)['steps'][0]
        self.assertEqual(row['status'],'completed')
        self.assertEqual(row['production']['layout'][0]['assigned'],3)
        state['sample']['country_overview']['production']['lines'][0]['requested_factories']=9
        self.assertEqual(evaluate(g,state)['steps'][0]['status'],'active')
        state['usable']=False
        self.assertNotEqual(evaluate(g,state)['steps'][0]['status'],'completed')
        # Both implementations must return identical full runtime contracts.
        cases=[]
        for usable,requested in [(True,10),(True,9),(False,10)]:
            current=copy.deepcopy(state);current['usable']=usable
            current['sample']['country_overview']['production']['lines'][0]['requested_factories']=requested
            cases.append({'guide':g,'state':current})
        js="const fs=require('fs'),r=require('./src/hoi4companion/web/guide-runtime.js');process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>r.evaluate(x.guide,x.state))));"
        output=subprocess.run(['node','-e',js],input=json.dumps(cases),capture_output=True,text=True,check=True)
        self.assertEqual([evaluate(c['guide'],c['state']) for c in cases],json.loads(output.stdout))

    def test_production_layout_rebinding_updates_targets_without_stale_conditions(self):
        catalog=[{'kind':'equipment','id':'rifle','name':'Rifles'}]
        g=definition({'category':'production','production_targets':[{'equipment_id':'rifle','requested_factories':10}]})
        bind_operations(g,catalog)
        g['tracks']['operations'][0]['action']['params']['production_targets'][0]['requested_factories']=12
        bind_operations(g,catalog)
        self.assertEqual(g['tracks']['operations'][0]['completion']['args'][0]['value'],12)

    def test_building_start_without_location_never_proves_completion(self):
        g=definition({'category':'construction','building_id':'arms_factory'})
        bind_operations(g,[{'kind':'building','id':'arms_factory','name':'Military Factory'}])
        state={'schema_version':1,'usable':True,'facts':{'construction.queued':{'status':'observed','source':'fixture','value':['building:arms_factory']}}}
        self.assertEqual(evaluate(g,state)['steps'][0]['status'],'active')
        state['facts']['construction.queued']['value']=[]
        self.assertNotEqual(evaluate(g,state)['steps'][0]['status'],'completed')

    def test_dates_validate_calendar_without_becoming_completion_conditions(self):
        for value in ['1936.2.29','1936.1.1']:
            normalise_guide(definition({'game_date':value}))
        for value in ['1937.2.29','1936.13.1','0.1.1','01:30',None]:
            with self.assertRaises(ValueError):normalise_guide(definition({'game_date':value}))

    def test_frames_require_unambiguous_source_and_real_timestamp(self):
        g=definition({'source_seconds':126});g['sources']=[{'id':'a','type':'youtube','url':'https://youtu.be/Ht4e89B02Wo'}]
        step=g['tracks']['operations'][0]
        self.assertEqual(frame_reference(g,step)['seconds'],126)
        g['sources'].append({'id':'b','type':'youtube','url':'https://youtu.be/FBPdhlwqMwM'})
        self.assertIsNone(frame_reference(g,step))
        with TemporaryDirectory() as d:
            with self.assertRaises(ValueError):GuideFrames(d).source('https://youtu.be/Ht4e89B02Wo')

    def test_video_split_hard_limit_variable_bitrate_and_overlap(self):
        with TemporaryDirectory() as d:
            root=Path(d);master=root/'master.mp4';master.write_bytes(b'x'*4000)
            calls=[]
            def encode(args,timeout):
                start=float(args[args.index('-ss')+1]);length=float(args[args.index('-t')+1]);calls.append((start,length))
                # A more expensive middle region exercises adaptive retries.
                Path(args[-1]).write_bytes(b'x'*int(length*(40 if start>=50 else 10)))
                return ''
            with patch('hoi4companion.guide_media.command',side_effect=encode),patch('hoi4companion.guide_media.duration',side_effect=lambda ff,path:calls[-1][1]):
                parts=split_video('ffmpeg',master,root,200,limit=1000)
            self.assertEqual(parts[0]['start_seconds'],0);self.assertEqual(parts[-1]['end_seconds'],200)
            self.assertTrue(all(p['bytes']<=1000 for p in parts))
            self.assertTrue(all(a['end_seconds']-b['start_seconds']==10 for a,b in zip(parts,parts[1:])))
            self.assertGreater(len(calls),len(parts))

    def test_video_parts_are_five_minutes_with_ten_second_overlap_even_below_size_cap(self):
        cases=[(30,[(0,30)]),(300,[(0,300)]),(301,[(0,300),(290,301)]),
               (590,[(0,300),(290,590)]),(620,[(0,300),(290,590),(580,620)])]
        for seconds,expected in cases:
            with self.subTest(seconds=seconds),TemporaryDirectory() as d:
                root=Path(d);master=root/'master.mp4';master.write_bytes(b'small source')
                lengths=[]
                def cut(args,timeout):
                    lengths.append(float(args[args.index('-t')+1]));Path(args[-1]).write_bytes(b'part')
                with patch('hoi4companion.guide_media.command',side_effect=cut),patch('hoi4companion.guide_media.duration',side_effect=lambda *args:lengths[-1]):
                    parts=split_video('ffmpeg',master,root,seconds)
                self.assertEqual([(p['start_seconds'],p['end_seconds']) for p in parts],expected)

    def test_video_cache_does_not_reuse_previous_segment_preset(self):
        with TemporaryDirectory() as d:
            root=Path(d);key='a'*32;folder=root/key;folder.mkdir();(folder/'parte-01.mp4').write_bytes(b'part')
            (folder/'manifest.json').write_text(json.dumps({'preset_version':2,'source_url':'https://www.youtube.com/watch?v=Ht4e89B02Wo',
                'fps':15,'max_part_bytes':150000000,'overlap_seconds':15,'parts':[{'name':'parte-01.mp4','bytes':4}]}))
            jobs=VideoJobs(root)
            with patch('hoi4companion.guide_media.threading.Thread') as worker:
                job=jobs.start('https://youtu.be/Ht4e89B02Wo')
                self.assertEqual(job['status'],'running');self.assertNotEqual(job['id'],key)
                worker.return_value.start.assert_called_once()

    def test_video_rejects_a_cut_that_seeks_to_an_earlier_keyframe(self):
        with TemporaryDirectory() as d:
            root=Path(d);master=root/'master.mp4';master.write_bytes(b'source')
            with patch('hoi4companion.guide_media.command',side_effect=lambda args,timeout:Path(args[-1]).write_bytes(b'part')),patch('hoi4companion.guide_media.duration',return_value=37):
                with self.assertRaisesRegex(ValueError,'allineato'):split_video('ffmpeg',master,root,30,limit=1000)

    def test_encoder_probe_falls_back_and_files_do_not_escape_job_directory(self):
        with patch('hoi4companion.guide_media.command',side_effect=[ValueError('driver'),ValueError('no device'),ValueError('no device'),'']):
            name,_,failed=choose_encoder('ffmpeg');self.assertEqual(name,'CPU x265');self.assertEqual(len(failed),3)
        with TemporaryDirectory() as d:
            jobs=VideoJobs(d)
            for url in ['file:///etc/passwd','https://localhost/a','https://youtube.com/playlist?list=a']:
                with self.assertRaises(ValueError):jobs.start(url)
            with self.assertRaises(ValueError):jobs.status('../anything')
            key='a'*32;jobs.jobs[key]={'status':'completed','parts':[{'name':'parte-01.mp4'}]}
            with self.assertRaises(ValueError):jobs.file(key,'../../settings.json')


if __name__=='__main__':unittest.main()
