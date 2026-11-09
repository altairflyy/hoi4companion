"""Independent native GUI decoder comparisons on synthetic process fixtures."""
import unittest
from copy import deepcopy
from hoi4companion.backend_core import _request
from hoi4companion.live import WORLD_RVA, LiveReadError
from test_focus_ui_native import UiFixture, FixtureReader, matrix


def observe(f,**options):
    profile={'world_rva':WORLD_RVA}
    for key in ('gui_layout','joint_focus_vtable','fixture_rtti'):
        if hasattr(f.profile,key):profile[key]=getattr(f.profile,key)
    bindings={key:f.bindings[key] for key in ('views','folders','focus','research','tabs_container','events') if key in f.bindings}
    if options.get('discover'):bindings={}
    return _request(0,'gui_fixture',memory=f.memory,classes=f.classes,base=f.base,pid=f.pid,created=f.created,
                    bindings=bindings,profile=profile,**options)


class NativeGuiObserverTests(unittest.TestCase):
    def registry_fixture(self):
        import struct
        f=UiFixture();f.visible_focus();f.visible_research()
        f.put(f.containers['focus']+0xA5,'<B',0)
        f.profile.fixture_rtti={}
        owners=list(f.bindings['views'].values())
        owners.append(f.object('CCountryTechnologyView'))
        owners.extend(a for panel in ('focus','research') for values in f.bindings[panel].values() for a in values)
        entries=[]
        for owner in owners:
            name=f.classes[struct.unpack('<Q',f.read(owner,8))[0]-f.base][4:-2]
            off=24 if name.endswith('Item') else 40
            secondary=f.object(name);vt=struct.unpack('<Q',f.read(secondary,8))[0]
            f.profile.fixture_rtti[str(vt)]=[f'.?AV{name}@@',off]
            f.put(owner+off,'<Q',vt)
            proxy=f.object('CButtonStandard');f.put(proxy+80,'<Q',owner+off);entries.append(proxy)
        folders=[]
        for name,c in list(f.bindings['folders'].items())+[('folder_tabs',f.bindings['tabs_container'])]:
            if name=='folder_tabs':
                definition=f.object('CContainerWindowType');f.string(definition+0x28,name);f.put(c+0x90,'<Q',definition)
            secondary=f.object('CContainerWindow');vt=struct.unpack('<Q',f.read(secondary,8))[0]
            f.put(c+48,'<Q',vt);f.profile.fixture_rtti[str(vt)]=['.?AVCContainerWindow@@',48]
            folders.append(c+48)
        f.array(f.containers['research']+0x10F0,folders)
        f.array(f.root+0x108,entries);f.array(f.root+0x120,[])
        return f

    def test_registry_discovery_binds_same_ids_without_supplied_bindings(self):
        f=self.registry_fixture()
        expected=FixtureReader(process=f).snapshot(focus_ids=['ROM_test'],technology_ids=['construction4'])
        actual=observe(f,discover=True,focus_ids=['ROM_test'],technology_ids=['construction4'])
        expected.pop('diagnostics');actual.pop('diagnostics')
        self.assertEqual(actual,expected)

    def assertObservation(self,f,**options):
        before=deepcopy(f.memory)
        expected=FixtureReader(process=f).snapshot(**options)
        actual=observe(f,**options)
        expected.pop('diagnostics');actual.pop('diagnostics')
        self.assertEqual(actual,expected);self.assertEqual(f.memory,before)
        return actual

    def test_focus_and_research_follow_current_parent_at_each_zoom(self):
        for panel in ('focus','research'):
            f=UiFixture();getattr(f,'visible_'+panel)()
            for scale in (1.,.5,.75,1.5):
                f.put(f.anchor+0x90,'<16f',*matrix(scale,500,-300))
                self.assertObservation(f,focus_ids=['ROM_test'],technology_ids=['construction4'])

    def test_hidden_widgets_and_other_folder_keep_observed_visibility(self):
        f=UiFixture();self.assertObservation(f)
        f.visible_research();self.assertObservation(f,technology_ids=['construction4'])
        f.put(f.research_outer+0x118,'<Q',f.folder_anchors['electronics_folder'])
        actual=self.assertObservation(f,technology_ids=['construction4'])
        self.assertFalse(actual['nodes'][0]['visible'])

    def test_foreign_country_tree_status_and_owner_fail_closed(self):
        for panel,mutate in (
            ('focus',lambda f:f.put(f.country+8,'<i',2)),
            ('focus',lambda f:f.put(f.focus+0x160,'<Q',f.country)),
            ('research',lambda f:f.put(f.model+0x158,'<Q',f.country)),
            ('research',lambda f:f.put(f.research_container+0x80,'<Q',f.country))):
            with self.subTest(panel=panel):
                f=UiFixture();getattr(f,'visible_'+panel)();mutate(f)
                with self.assertRaises(LiveReadError):FixtureReader(process=f).snapshot(focus_ids=['ROM_test'],technology_ids=['construction4'])
                with self.assertRaises(ValueError):observe(f,focus_ids=['ROM_test'],technology_ids=['construction4'])

    def test_invalid_transform_is_never_replaced_by_container_geometry(self):
        f=UiFixture();f.visible_research()
        f.put(f.anchor+0x90,'<16f',*matrix(0))
        with self.assertRaises(ValueError):observe(f,technology_ids=['construction4'])

