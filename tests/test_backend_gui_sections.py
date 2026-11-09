"""Native GUI sections reuse independent event/action/production fixtures."""
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.focus_ui_native import GuiSample
from hoi4companion.live import LiveReadError
from test_event_ui_native import EventUiFixture
from test_focus_ui_native import FixtureReader,UiFixture
from test_backend_gui_observer import observe


def section(ui,root,bindings,resolution,country,kind,**extra):
    f=ui.process
    profile={key:getattr(f.profile,key) for key in ('sha256','gui_layout','production_owner_offset') if hasattr(f.profile,key)}
    profile['fixture_rtti']={str(f.base+rva):ui._rtti(f.base+rva) for rva in f.classes}
    return _request(0,'gui_fixture',memory=f.memory,classes=f.classes,base=f.base,root=root,bindings=bindings,
                    profile=profile,resolution=resolution,country=country,section=kind,**extra)


class NativeGuiSectionsTests(unittest.TestCase):
    def test_event_popup_occlusion_and_rendered_answers_match(self):
        for requested in (['example.1'],['unrelated.1'],[]):
            f=EventUiFixture();f.event()
            ui=FixtureReader(process=f)
            expected=f.read_events(requested)
            actual=section(ui,f.root,f.bindings,[2560,1440],f.country,'events',event_ids=requested)
            self.assertEqual(actual,expected)
            expected=ui.snapshot(event_ids=requested);actual=observe(f,event_ids=requested)
            expected.pop('diagnostics');actual.pop('diagnostics');self.assertEqual(actual,expected)

    def test_event_foreign_option_and_duplicate_row_fail_closed(self):
        f=EventUiFixture();data=f.event();f.array(data['event']+0x368,[])
        with self.assertRaises(LiveReadError):f.read_events()
        with self.assertRaises(ValueError):section(FixtureReader(process=f),f.root,f.bindings,[2560,1440],f.country,'events',event_ids=['example.1'])

    def test_production_rows_and_picker_use_exact_native_membership(self):
        from test_production_controls_native import ProductionControlTests
        for picker in (False,True):
            fixture=ProductionControlTests();fixture.setUp()
            if picker:fixture.open_picker()
            expected=fixture.fixture.snapshot()
            actual=section(fixture.ui,fixture.fixture.root,fixture.bindings,[2560,1440],fixture.f.country,'production')
            # Tuple/list distinction belongs only to the Python transport.
            import json
            self.assertEqual(actual,json.loads(json.dumps(expected)))

    def test_unknown_production_build_does_not_reuse_another_layout(self):
        f=UiFixture();ui=FixtureReader(process=f)
        self.assertEqual(section(ui,f.root,{},[2560,1440],f.country,'production'),{'status':'unavailable','reason':'Production GUI profile unavailable'})

    def test_common_action_controls_and_partial_coverage_match(self):
        from hoi4companion.action_ui_native import read_controls
        from hoi4companion.live import NativeSample
        f=UiFixture();f.research_tabs();ui=FixtureReader(process=f)
        c=f.bindings['tabs_container'];f.put(c+0xA5,'<B',8)
        definition=f.object('CContainerWindowType');f.string(definition+0x28,'topbar');f.put(c+0x90,'<Q',definition)
        button=NativeSample(f).array(c+0x1120,maximum=256)[0]
        f.string(NativeSample(f).number(button+0x60)+0x28,'player_flag')
        for shown in (8,0):
            f.put(c+0xA5,'<B',shown);roots={c:'topbar'}
            expected=read_controls(ui,GuiSample(f),f.root,roots,[2560,1440],f.country)
            actual=section(ui,f.root,f.bindings,[2560,1440],f.country,'actions',roots=roots)
            self.assertEqual(actual,expected)
