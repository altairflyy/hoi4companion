import unittest
from hoi4companion.country import CountryReader
from hoi4companion.events_native import read_events, read_decisions, GUI_RVA
from hoi4companion.live import LiveReadError, compile_watch_reads, watch_changed
from test_native_fixture import Fixture


class EventFixture(Fixture):
    def __init__(self):
        super().__init__(); self.root=self.object('CGameGui'); self.put(self.base+GUI_RVA,'<Q',self.root)
        self.owner=self.object('CEventWindow'); self.container=self.object('CContainerWindow')
        self.put(self.owner+0x50,'<Q',self.container); self.put(self.container+0x30+104,'<Q',self.root)
        self.put(self.container+0x30+117,'<B',8);self.put(self.container+0x30+80,'<Q',self.owner+56)
        self.array(self.root+264,[]);self.array(self.root+288,[])
        self._hoi4_event_cache=(self.root,[self.owner],tuple(self.read(self.root+offset,16) for offset in (264,288)))
        self.event=self.object('CEvent'); self.put(self.owner+0x68,'<Q',self.event); self.string(self.event+0x20,'example.1')
        self.grid=self.object('CStandardGridBox'); self.put(self.owner+0x1280,'<Q',self.grid)
        self.option=self.object('CEventOption'); self.string(self.option+0x178,'example.1.a')
        self.array(self.event+0x368,[self.option]); self.put(self.option+0x1f0,'<i',0)
        self.row=self.object('CStandardGridBoxItem'); self.array(self.grid+0x1a0,[self.row])
        self.item=self.object('CContainerWindow'); self.put(self.row+16,'<Q',self.item)
        self.button=self.object('CButtonDrag'); self.array(self.item+0x10d8,[self.button]); self.put(self.button+8,'<Q',self.option)

    def reader(self): return CountryReader(self,self.world,self.country,1)


class EventNativeTests(unittest.TestCase):
    def test_displayed_grid_filters_definition_options_and_watches_pause_changes(self):
        f=EventFixture(); hidden=f.object('CEventOption'); f.string(hidden+0x178,'example.1.b');f.put(hidden+0x1f0,'<i',1);f.array(f.event+0x368,[f.option,hidden])
        r=f.reader(); result=read_events(r); r.s.verify()
        self.assertEqual(result['visible'][0]['option_ids'],['example.1.a'])
        self.assertEqual(result['visible'][0]['event_id'],'example.1')
        reads=compile_watch_reads(r.watch); self.assertFalse(watch_changed(f,reads))
        f.put(f.container+0x30+117,'<B',0);self.assertTrue(watch_changed(f,reads))
        self.assertEqual(read_events(f.reader())['visible'],[])

    def test_foreign_option_not_in_definition_rejected_and_no_prefix_published(self):
        f=EventFixture();other=f.object('CEventOption');f.string(other+0x178,'other.1.a');f.put(f.button+8,'<Q',other)
        with self.assertRaises(LiveReadError):read_events(f.reader())

    def test_wrong_root_and_duplicate_rows_rejected(self):
        for kind in ['root','duplicate']:
            f=EventFixture()
            if kind=='root':f.put(f.container+0x30+104,'<Q',f.world)
            else:f.array(f.grid+0x1a0,[f.row,f.row])
            with self.assertRaises(LiveReadError):read_events(f.reader())

    def test_current_event_not_retained_when_window_hidden(self):
        f=EventFixture(); f.put(f.container+0x30+117,'<B',0)
        self.assertEqual(read_events(f.reader())['visible'],[])

    def test_decision_membership_does_not_claim_availability_or_completion(self):
        f=Fixture();manager=f.object('CDecisionStatus');f.put(f.country+0xfa0,'<Q',manager);f.put(manager+8,'<Q',f.country)
        item=f.object('CDecision');f.string(item+0x120,'war_bonds');f.array(manager+0x40,[item]);f.array(manager+0x148,[])
        r=CountryReader(f,f.world,f.country,1);result=read_decisions(r);r.s.verify()
        self.assertEqual(result['listed_ids'],['war_bonds']);self.assertNotIn('active_ids',result);self.assertNotIn('completed_ids',result)
        f.put(manager+8,'<Q',f.world)
        with self.assertRaises(LiveReadError):read_decisions(CountryReader(f,f.world,f.country,1))
