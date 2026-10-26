import unittest
from hoi4companion.action_ui_native import discover_roots, read_controls, payload_identity
from hoi4companion.live import NativeSample
from test_focus_ui_native import UiFixture, FixtureReader


class ActionUiTests(unittest.TestCase):
    def test_common_button_geometry_is_live_and_hidden_or_foreign_owners_fail_closed(self):
        f=UiFixture();f.research_tabs();ui=FixtureReader(process=f)
        container=f.bindings['tabs_container'];f.put(container+0xA5,'<B',8)
        definition=f.object('CContainerWindowType');f.string(definition+0x28,'topbar');f.put(container+0x90,'<Q',definition)
        button=NativeSample(f).array(container+0x1120,maximum=256)[0]
        f.string(NativeSample(f).number(button+0x60)+0x28,'player_flag')
        bindings={'registry_seen':{container}}
        roots=discover_roots(ui,f.root,bindings);self.assertEqual(roots,{container:'topbar'})
        def read():return read_controls(ui,NativeSample(f),f.root,roots,[2560,1440],f.country)
        value=read();self.assertEqual(value['status'],'partial') # other panel visibility unknown
        self.assertEqual(len(value['controls']),1)
        rect=value['controls'][0]['rect'];self.assertGreater(rect[2],0)
        f.put(container+0xA5,'<B',0);self.assertEqual(read()['controls'],[])
        f.put(container+0xA5,'<B',8);f.put(container+0x98,'<Q',f.root+1)
        self.assertEqual(read()['status'],'unavailable')

    def test_exact_law_group_and_advisor_country_are_not_inferred_from_text(self):
        f=UiFixture();ui=FixtureReader(process=f);button=f.object('CButtonStandard')
        group=f.object('CIdeaGroupType');f.string(group+0x38,'economy');f.put(group+0x58,'<B',1)
        idea=f.object('CIdea');f.string(idea+0x18,'war_economy');f.put(idea+0xAA0,'<Q',group);f.put(button+8,'<Q',idea)
        value=payload_identity(ui,NativeSample(f),button,f.country)
        self.assertEqual(value,{'entity_kind':'law','entity_id':'war_economy','category_id':'economy'})
        advisor=f.object('CAdvisor');person=f.object('CCharacter');f.string(advisor+0x30,'infantry_expert')
        f.string(person+0x48,'person');f.put(person+0x68,'<i',1);f.put(advisor+0x10,'<Q',person);f.put(button+8,'<Q',advisor)
        self.assertEqual(payload_identity(ui,NativeSample(f),button,f.country)['entity_id'],'infantry_expert')
        f.put(person+0x68,'<i',2);self.assertEqual(payload_identity(ui,NativeSample(f),button,f.country),{})

    def test_action_request_reaches_the_witnessed_snapshot(self):
        ui=FixtureReader(process=UiFixture());calls=[]
        ui._snapshot=lambda **args: calls.append(args) or {'diagnostics':{}}
        ui.snapshot(action_ui=True)
        self.assertIs(calls[0]['action_ui'],True)
