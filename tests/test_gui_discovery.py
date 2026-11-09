"""Cold discovery keeps all candidates; final geometry uses separate witnesses."""
from collections import Counter
import struct
import unittest
from hoi4companion.focus_ui_native import UiReader
from hoi4companion.live import NativeSample,LiveReadError
from test_native_fixture import Fixture


class Registry(Fixture):
    def __init__(self):
        super().__init__();self.root=self.object('CGameGui');self.secondary={};self.entries=[]
        self.reads=Counter();self.bad=0x300000000;self.unreadable=True
        self.views={}
        for kind in ('CNationalFocusView','CCountryTechnologyView','CCountryTechTreeView'):
            owner=self.object(kind);self.views[kind]=owner;self.register(owner,kind)
        owner=self.views['CCountryTechTreeView'];container=self.object('CContainerWindow')
        self.put(owner+0x578,'<Q',container)
        definition=self.object('CContainerWindowType');self.string(definition+0x28,'countrytechtreeview')
        self.put(container+0x90,'<Q',definition)
        self.folder=self.object('CContainerWindow');folder_gui=self.folder+48
        self.put(folder_gui,'<Q',self.vtable('CContainerWindow',48))
        definition=self.object('CContainerWindowType');self.string(definition+0x28,'industry_folder')
        self.put(folder_gui+0x60,'<Q',definition);self.array(container+0x10F0,[folder_gui])
        focus=self.object('CNationalFocusTreeItem');self.put(focus+0x20,'<Q',self.focus)
        self.register(focus,'CNationalFocusTreeItem',24)
        technology=self.object('CTechnologyTreeTechItem');self.put(technology+0xC8,'<Q',self.model)
        self.register(technology,'CTechnologyTreeTechItem',24)
        topbar=self.object('CTopBar');self.register(topbar,'CTopBar')
        self.button=self.object('CButtonStandard');self.entries.append(self.button)
        self.put(self.button+0x50,'<Q',topbar+40)
        definition=self.object('CButtonType');self.string(definition+0x28,'production_button')
        self.put(self.button+0x60,'<Q',definition)
        for i in range(96):self.entries.append(self.bad+i*16)
        # Repeated invalid immutable vtables must be decoded only once.
        self.unknown_vtable=self.base+0x1FF000
        for _ in range(24):
            gui=self.object('CIcon');observer=self.allocate()
            self.put(gui+80,'<Q',observer);self.put(observer,'<Q',self.unknown_vtable);self.entries.append(gui)
        self.array(self.root+0x108,self.entries);self.array(self.root+0x120,[])

    def vtable(self,kind,offset):
        address=self.base+0x100000+len(self.secondary)*32
        self.secondary[address]=('.?AV'+kind+'@@',offset);return address

    def register(self,owner,kind,offset=40):
        self.put(owner+offset,'<Q',self.vtable(kind,offset))
        gui=self.object('CIcon');self.put(gui+80,'<Q',owner+offset);self.entries.append(gui)

    def read(self,address,size):
        if hasattr(self,'reads'):
            self.reads[(address,size)]+=1
            if self.unreadable and self.bad<=address<self.bad+4096:raise OSError('unmapped page')
        return super().read(address,size)


class Reader(UiReader):
    def _rtti(self,vt):
        self.resolutions[vt]+=1
        if vt in self.process.secondary:return self.process.secondary[vt]
        name=self.process.classes.get(vt-self.process.base)
        if name:return name,0
        raise LiveReadError('Invalid fixture RTTI')

    def __init__(self,process):
        self.resolutions=Counter();super().__init__(process=process)


class GuiDiscoveryTests(unittest.TestCase):
    def test_cold_scan_preserves_views_focus_research_and_production(self):
        f=Registry();reader=Reader(f);result=reader._discover(NativeSample(f),f.root)
        self.assertEqual(result['views'],f.views)
        self.assertEqual(list(result['focus']),['ROM_test'])
        self.assertEqual(list(result['research']),['construction4'])
        self.assertEqual(result['folders'],{'industry_folder':f.folder})
        self.assertEqual(result['production_access'],[f.button])
        self.assertEqual(f.reads[(f.bad,4096)],1)
        self.assertEqual(reader.resolutions[f.unknown_vtable],1)

    def test_candidate_pages_and_failure_cache_are_discarded_after_discovery(self):
        f=Registry();reader=Reader(f);reader._discover(NativeSample(f),f.root)
        f.unreadable=False;reader._cache=None
        f.string(f.focus+0x18,'changed_focus')
        result=reader._discover(NativeSample(f),f.root)
        self.assertEqual(list(result['focus']),['changed_focus'])
        self.assertEqual(f.reads[(f.bad,4096)],2)
        # Neither candidate bytes nor a cached pointer are observations.
        sample=NativeSample(f);sample.read(f.focus+0x18,8);f.put(f.focus+0x18,'<Q',0)
        with self.assertRaises(LiveReadError):sample.verify()

    def test_vtables_outside_qualified_image_never_resolve_as_game_types(self):
        f=Registry();f.profile.image_size=0x200000
        reader=Reader(f);reader._discover(NativeSample(f),f.root)
        self.assertTrue(all(f.base<=vt<f.base+f.profile.image_size for vt in reader.resolutions))

    def test_invalid_and_changed_registry_headers_keep_the_original_guards(self):
        f=Registry();reader=Reader(f);reader._discover(NativeSample(f),f.root)
        f.put(f.root+0x120,'<Qii',0,4,5)
        with self.assertRaises(LiveReadError):reader._discover(NativeSample(f),f.root)

    def test_cleared_slots_never_follow_old_observer_bytes_or_enter_second_pass(self):
        f=Registry();gui=f.object('CIcon');observer=f.allocate()
        f.put(gui,'<Q',0);f.put(gui+80,'<Q',observer)
        f.put(observer,'<Q',f.unknown_vtable);f.entries.append(gui);f.array(f.root+0x108,f.entries)
        reader=Reader(f);reader._discover(NativeSample(f),f.root)
        self.assertEqual(f.reads[(observer & ~4095,4096)],0)
        self.assertLess(reader.discovery_reads['non_null_widgets'],reader.discovery_reads['registry_objects'])
