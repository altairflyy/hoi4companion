import struct
import unittest

from hoi4companion.event_ui_native import read_event_ui, refresh_candidates
from hoi4companion.focus_ui_native import GuiSample
from hoi4companion.live import LiveReadError
from test_focus_ui_native import FixtureReader, UiFixture, matrix


class EventUiFixture(UiFixture):
    def event(self, identifier='example.1', option_id='example.1.a', index=0):
        owner = self.object('CEventWindow')
        container = self.object('CContainerWindow')
        self.put(owner+0x50, '<Q', container)
        self.put(container+0x98, '<Q', self.root); self.put(container+0x80, '<Q', owner+56)
        self.put(container+0xA5, '<B', 8)
        self.put(container+0x1594, '<4i', 0, 0, 2560, 1440)
        self.put(container+0x15AC, '<ii', 500, 600)
        position = self.object('C2dPositionObject'); self.put(container+0x15F0, '<Q', position)
        self.put(position+0x90, '<16f', *matrix(1, 800, -200))
        self.put(position+0x130, '<ii', 2560, 1440)
        event = self.object('CEvent'); self.put(owner+0x68, '<Q', event)
        self.string(event+0x20, identifier)
        option = self.object('CEventOption')
        self.string(option+0x178, option_id); self.put(option+0x1F0, '<i', index)
        self.array(event+0x368, [option])
        grid = self.object('CStandardGridBox'); self.put(owner+0x1280, '<Q', grid)
        self.put(grid+0x68, '<Q', self.root); self.put(grid+0x50, '<Q', owner+56)
        row = self.object('CStandardGridBoxItem')
        self.array(grid+0x1A0, [row])
        item = self.object('CContainerWindow'); self.put(row+16, '<Q', item)
        self.put(item+0x98, '<Q', self.root); self.put(item+0x80, '<Q', owner+56)
        self.put(item+0xA5, '<B', 8)
        parent = self.object('C2dPositionObject'); self.put(item+0x15F0, '<Q', parent)
        self.put(parent+0x90, '<16f', *matrix(1, 900, -700))
        button = self.object('CButtonDrag'); self.array(item+0x10D8, [button])
        self.put(button+8, '<Q', option)
        self.put(button+0x68, '<Q', self.root); self.put(button+0x50, '<Q', owner+56)
        self.put(button+0x75, '<B', 8); self.put(button+0x110, '<Q', parent)
        sprite = self.object('CSprite'); self.put(button+0x118, '<Q', sprite)
        self.put(sprite+0x118, '<Q', parent); self.put(sprite+0x128, '<2f', 0, 0)
        self.put(sprite+0x124, '<f', 1)
        # The old rendering matrix is not a source of overlay coordinates.
        self.put(sprite+0x90, '<16f', *matrix(1, -9999, 9999))
        texture = self.object('CSpriteType'); self.put(sprite+0x180, '<Q', texture)
        self.put(texture+0x90, '<ii', 1, 1); self.put(texture+0xA4, '<ii', 300, 47)
        self.bindings.setdefault('events', []).append(owner)
        return dict(owner=owner, container=container, position=position, event=event,
                    option=option, grid=grid, row=row, item=item, parent=parent,
                    button=button, sprite=sprite)

    def read_events(self, event_ids=('example.1',)):
        reader = FixtureReader(process=self); sample = GuiSample(self)
        result = read_event_ui(reader, sample, self.root, self.bindings.get('events', []), set(event_ids), [2560,1440])
        sample.verify()
        return result


class EventUiNativeTests(unittest.TestCase):
    def test_native_popup_and_rendered_button_have_exact_ids_and_bounds(self):
        f = EventUiFixture(); data=f.event()
        result = f.read_events()
        self.assertEqual(result['visible_ids'], ['example.1'])
        node = result['nodes'][0]
        self.assertEqual(node['identity'], 'example.1:example.1.a')
        self.assertEqual(node['instance_id'], format(data['owner'], 'x'))
        self.assertEqual(node['rect'], [900.,700.,300.,47.])
        self.assertEqual(result['event_windows'][0]['rect'], [800.,200.,500.,600.])

    def test_definition_only_option_does_not_become_rendered_choice(self):
        f=EventUiFixture(); data=f.event(); hidden=f.object('CEventOption')
        f.string(hidden+0x178,'example.1.b'); f.put(hidden+0x1F0,'<i',1)
        f.array(data['event']+0x368,[data['option'],hidden])
        self.assertEqual([n['option_id'] for n in f.read_events()['nodes']],['example.1.a'])

    def test_unrequested_popup_has_priority_without_option_disclosure(self):
        f=EventUiFixture(); f.event()
        result=FixtureReader(process=f).snapshot(event_ids=['other.1'])
        self.assertEqual(result['active_panel'],'event')
        self.assertTrue(result['panels']['event']['visible'])
        self.assertEqual(result['nodes'],[])
        self.assertEqual(result['event_windows'][0]['event_id'],'example.1')

    def test_popup_priority_over_existing_focus_is_exactly_native(self):
        f=EventUiFixture(); f.visible_focus(); f.event()
        result=FixtureReader(process=f).snapshot(focus_ids=['ROM_test'],event_ids=['example.1'])
        self.assertEqual(result['active_panel'],'event')
        self.assertEqual({n['panel'] for n in result['nodes']},{'focus','event'})
        # Parent can suppress tree marks/occlude them without losing the raw
        # native focus observation; no event answer or step is completed here.

    def test_closed_popup_is_hidden_without_claiming_a_response(self):
        f=EventUiFixture(); data=f.event(); f.put(data['container']+0xA5,'<B',0)
        result=f.read_events()
        self.assertEqual(result['status'],'unknown')
        self.assertEqual(result['event_windows'],[])
        self.assertEqual(result['nodes'],[])

    def test_foreign_option_and_foreign_button_owner_fail_closed(self):
        f=EventUiFixture(); data=f.event(); foreign=f.object('CEventOption')
        f.put(data['button']+8,'<Q',foreign)
        with self.assertRaisesRegex(LiveReadError,'estranea'):f.read_events()
        f=EventUiFixture(); data=f.event(); f.put(data['button']+0x50,'<Q',data['owner']+40)
        with self.assertRaisesRegex(LiveReadError,'altro evento'):f.read_events()

    def test_duplicate_option_indices_in_rendered_grid_are_rejected(self):
        f=EventUiFixture(); data=f.event()
        # Two rendered rows with the same option are ambiguous, even if the
        # event definition contains that option exactly once.
        f.array(data['grid']+0x1A0,[data['row'],data['row']])
        with self.assertRaisesRegex(LiveReadError,'duplicati'):f.read_events()

    def test_movement_and_zoom_follow_current_button_parent(self):
        f=EventUiFixture(); data=f.event(); f.read_events()
        f.put(data['parent']+0x90,'<16f',*matrix(.5,1000,-500))
        self.assertEqual(f.read_events()['nodes'][0]['rect'],[1000.,500.,150.,23.5])

    def test_different_popup_resolution_and_sprite_parent_are_rejected(self):
        f=EventUiFixture(); data=f.event(); f.put(data['position']+0x130,'<ii',1920,1080)
        with self.assertRaisesRegex(LiveReadError,'Risoluzione'):f.read_events()
        f=EventUiFixture(); data=f.event(); f.put(data['sprite']+0x118,'<Q',f.object('C2dPositionObject'))
        with self.assertRaisesRegex(LiveReadError,'Ancoraggio'):f.read_events()

    def test_two_instances_of_same_event_are_preserved_for_parent_ambiguity_gate(self):
        f=EventUiFixture(); f.event(); f.event()
        result=f.read_events()
        self.assertEqual(len(result['nodes']),2)
        self.assertEqual(len({n['instance_id'] for n in result['nodes']}),2)

    def test_no_candidate_is_unknown_instead_of_invented_absence(self):
        f=EventUiFixture()
        self.assertEqual(f.read_events()['status'],'unknown')

    def test_instance_hint_is_only_a_candidate_and_is_fully_qualified(self):
        f=EventUiFixture(); data=f.event(); f.bindings['events']=[]
        result=FixtureReader(process=f).snapshot(event_ids=['example.1'], event_instances=[format(data['owner'],'x')])
        self.assertEqual(result['nodes'][0]['identity'],'example.1:example.1.a')
        f.put(data['container']+0x98,'<Q',f.object('CGameGui'))
        with self.assertRaisesRegex(LiveReadError,'Proprietario'):
            FixtureReader(process=f).snapshot(event_ids=['example.1'],event_instances=[format(data['owner'],'x')])

    def test_torn_button_geometry_is_rejected_before_publishing(self):
        f=EventUiFixture(); data=f.event(); reader=FixtureReader(process=f); sample=GuiSample(f)
        read_event_ui(reader,sample,f.root,[data['owner']],{'example.1'},[2560,1440])
        f.put(data['parent']+0x90,'<16f',*matrix(1,1200,-700))
        with self.assertRaisesRegex(LiveReadError,'incoerente'):sample.verify()

    def test_new_dynamic_registry_entry_discovers_missing_popup_once(self):
        f=EventUiFixture(); data=f.event(); f.bindings['events']=[]
        old_static=struct.pack('<Qii',f.allocate(),0,0)
        dynamic=f.allocate(); old_dynamic=struct.pack('<Qii',dynamic,1,0)
        f.put(f.root+0x108,'<Qii',struct.unpack('<Q',old_static[:8])[0],0,0)
        f.put(f.root+0x120,'<Qii',dynamic,1,1)
        gui=f.allocate(); f.put(dynamic,'<Q',gui); f.put(gui+80,'<Q',data['owner'])
        f.bindings['registry_headers']=(old_static,old_dynamic)
        f.bindings['registry_seen']=set()
        reader=FixtureReader(process=f)
        self.assertEqual(refresh_candidates(reader,f.root,f.bindings),[data['owner']])
        calls=[]; read=f.read
        def log_read(address,size):calls.append((address,size));return read(address,size)
        f.read=log_read
        self.assertEqual(refresh_candidates(reader,f.root,f.bindings),[data['owner']])
        self.assertEqual(calls,[(f.root+0x108,16),(f.root+0x120,16)])

    def test_invalid_or_foreign_instance_hint_never_supplies_identity(self):
        f=EventUiFixture(); reader=FixtureReader(process=f)
        for value in ('nothex',' 10000','0x10000','0','ffffffffffffffff'):
            with self.subTest(value=value), self.assertRaises(LiveReadError):
                refresh_candidates(reader,f.root,f.bindings,[value])

    def test_missing_grid_button_and_row_owner_are_rejected(self):
        f=EventUiFixture(); data=f.event(); f.array(data['item']+0x10D8,[])
        with self.assertRaisesRegex(LiveReadError,'pulsante'):f.read_events()
        f=EventUiFixture(); data=f.event(); f.put(data['item']+0x80,'<Q',data['owner']+40)
        with self.assertRaisesRegex(LiveReadError,'Proprietario'):f.read_events()


if __name__=='__main__':unittest.main()
