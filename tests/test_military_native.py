"""Fault and adapter tests; these fixtures are not a live game observation."""
import copy
import json
from pathlib import Path
import subprocess
import unittest

from hoi4companion.country import CountryReader
from hoi4companion.contracts import normalise_state
from hoi4companion.live import LiveReadError, MILITARY_LAYOUT
from test_native_fixture import Fixture


class MilitaryNativeTests(unittest.TestCase):
    def fixture(self):
        f=Fixture()
        f.profile.military_layout=MILITARY_LAYOUT
        f.profile.bytes=lambda address,size:dict(MILITARY_LAYOUT['signatures'])[address][:size]
        exp=f.object('CCountryExperienceStatus')
        f.put(f.country+0x1588,'<Q',exp);f.put(exp+8,'<Q',f.country)
        for off in (0x10,0x28,0x40):f.put(exp+off,'<q',0)
        dip=f.object('CDiplomacyStatus')
        f.put(f.country+0xF88,'<Q',dip);f.put(dip+0x2D0,'<Q',f.country)
        f.array(dip+0x98,[]);f.array(dip+0x20,[])
        return f,exp,dip

    def reader(self,f):return CountryReader(f,f.world,f.country,1)

    def test_experience_fixed_point_scale_zero_fraction_and_independent_balances(self):
        f,exp,_=self.fixture()
        f.put(exp+0x10,'<q',20*32768+16384)
        f.put(exp+0x40,'<q',32768)
        r=self.reader(f)
        self.assertEqual(r.experience(),{'status':'observed','army':20.5,'navy':0,'air':1})
        self.assertIn((exp+0x10,8),r.watch)
        r.s.verify()
        f.put(exp+0x10,'<q',21*32768)
        with self.assertRaises(LiveReadError):r.s.verify()

    def test_bad_owner_identity_signature_and_range_are_rejected(self):
        for mutation in (
            lambda f,e,d:f.put(e+8,'<Q',f.country+0x2000),
            lambda f,e,d:f.put(f.country+8,'<i',2),
            lambda f,e,d:f.put(e+0x28,'<q',-1),
            lambda f,e,d:f.put(e+0x40,'<q',32768*10_000_001),
            lambda f,e,d:setattr(f.profile,'bytes',lambda address,size:b'\0'*size),
        ):
            f,e,d=self.fixture();mutation(f,e,d)
            with self.assertRaises(LiveReadError):self.reader(f).experience()
        f,e,d=self.fixture();del f.profile.military_layout
        with self.assertRaises(LiveReadError):self.reader(f).experience()

    def test_peace_empty_war_list_and_pending_war_relation(self):
        f,_,dip=self.fixture()
        self.assertFalse(self.reader(f).diplomacy()['at_war'])
        f.array(dip+0x98,[f.object('CWar')])
        self.assertTrue(self.reader(f).diplomacy()['at_war'])
        f.array(dip+0x98,[])
        relation=f.object('CWarRelation');f.put(relation+8,'<i',0x380A)
        f.array(dip+0x20,[relation])
        r=self.reader(f)
        self.assertTrue(r.diplomacy()['at_war'])
        self.assertIn((relation+8,4),r.watch)

    def test_unrelated_relations_do_not_mean_war_and_invalid_ownership_is_not_peace(self):
        f,_,dip=self.fixture()
        relation=f.object('CRelation');f.put(relation+8,'<i',0x1234)
        f.array(dip+0x20,[relation])
        self.assertFalse(self.reader(f).diplomacy()['at_war'])
        f.put(relation+8,'<i',0x380A)
        with self.assertRaises(LiveReadError):self.reader(f).diplomacy()
        f,_,dip=self.fixture();f.put(dip+0x2D0,'<Q',f.country+0x2000)
        with self.assertRaises(LiveReadError):self.reader(f).diplomacy()

    def test_contracts_reject_unsupported_types_partial_foreign_and_stale_python_js_parity(self):
        state={'usable':True,'sample':{'country':'TST','country_overview':{'country':'TST',
            'experience':{'status':'observed','army':20.5,'navy':0,'air':1},
            'diplomacy':{'status':'observed','at_war':False},
            'production':{'status':'observed','lines':[],'effective_factories':{'military':12}}}}}
        states=[state]
        for key,value in [('experience',{'status':'partial','army':20}),('diplomacy',{'status':'observed','at_war':0}),
                          ('experience',{'status':'observed','army':True,'navy':-1,'air':'20'}),
                          ('production',{'status':'partial','lines':[],'effective_factories':{'military':12}})]:
            clone=copy.deepcopy(state);clone['sample']['country_overview'][key]=value;states.append(clone)
        clone=copy.deepcopy(state);clone['sample']['country_overview']['country']='ALT';states.append(clone)
        clone=copy.deepcopy(state);clone['usable']=False;states.append(clone)
        results=[normalise_state(s) for s in states]
        self.assertEqual(results[0]['facts']['country.experience.navy']['value'],0)
        self.assertEqual(results[1]['facts']['country.experience.army']['status'],'unknown')
        self.assertEqual(results[2]['facts']['country.diplomacy.at_war']['status'],'unknown')
        self.assertEqual(results[3]['facts']['country.experience.army']['status'],'unknown')
        self.assertEqual(results[4]['facts']['country.production.effective_factories.military']['status'],'unknown')
        module=(Path(__file__).parent.parent/'src/hoi4companion/web/contracts.js').as_posix()
        script='const c=require(process.argv[1]);let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>process.stdout.write(JSON.stringify(JSON.parse(s).map(c.state))));'
        result=subprocess.run(['node','-e',script,module],input=json.dumps(states),text=True,capture_output=True,check=True)
        # Compare the exact meaningful facts. Legacy consumers may phrase unknown reasons differently.
        js=json.loads(result.stdout)
        keys=('country.experience.army','country.experience.navy','country.experience.air',
              'country.diplomacy.at_war','country.production.effective_factories.military')
        for py,j in zip(results,js):
            self.assertEqual(py['usable'],j['usable'])
            for key in keys:
                self.assertEqual(py['facts'].get(key,{}).get('value'),j['facts'].get(key,{}).get('value'))
                self.assertEqual(py['facts'].get(key,{}).get('status'),j['facts'].get(key,{}).get('status'))
