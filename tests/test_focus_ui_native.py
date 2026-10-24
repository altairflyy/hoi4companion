import math
import struct
import unittest

from hoi4companion.focus_ui_native import UiReader, geometry, sprite_geometry, anchored_sprite_geometry
from hoi4companion.events_native import GUI_RVA
from hoi4companion.live import LiveReadError, NativeSample
from test_native_fixture import Fixture


def matrix(scale=1., x=0., y=0.):
    return [scale, 0., 0., 0., 0., scale, 0., 0., 0., 0., 1., 0., x, y, 0., 1.]


class FixtureReader(UiReader):
    def _rtti(self, vt):
        return self.profile.class_name(vt-self.process.base), 0

    def _discover(self, sample, root):
        return self.process.bindings


class UiFixture(Fixture):
    def __init__(self):
        super().__init__()
        self.root = self.object('CGameGui')
        self.put(self.base+GUI_RVA, '<Q', self.root)
        self.bindings = {'views': {}, 'focus': {}, 'research': {}, 'folders': {}, 'tabs_container': None}
        self.containers = {}
        for panel, name in [('focus', 'CNationalFocusView'), ('research', 'CCountryTechTreeView')]:
            owner = self.object(name)
            self.bindings['views'][name] = owner
            container = self.object('CContainerWindow')
            self.containers[panel] = container
            self.put(owner+0x578, '<Q', container)
            self.put(container+0x98, '<Q', self.root)
            self.put(container+0x80, '<Q', owner+40)
            self.put(container+0xA5, '<B', 0)
            self.put(container+0x1594, '<4i', 0, 78, 2560, 1440)
            pos = self.object('C2dPositionObject')
            self.put(container+0x15F0, '<Q', pos)
            self.put(pos+0x130, '<ii', 2560, 1440)
            if panel == 'research':
                definition = self.object('CContainerWindowType')
                self.put(container+0x90, '<Q', definition)
                self.string(definition+0x28, 'countrytechtreeview')

    def visible_research(self):
        view = self.bindings['views']['CCountryTechTreeView']
        self.put(self.containers['research']+0xA5, '<B', 8)
        self.folder_anchors = {}
        for name, shown in [('industry_folder', True), ('electronics_folder', False)]:
            container = self.object('CContainerWindow')
            self.bindings['folders'][name] = container
            self.put(container+0x98, '<Q', self.root)
            self.put(container+0x80, '<Q', view+40)
            self.put(container+0xA5, '<B', 8 if shown else 0)
            definition = self.object('CContainerWindowType')
            self.put(container+0x90, '<Q', definition); self.string(definition+0x28, name)
            self.put(container+0x1594, '<4i', 10, 138, 2535, 1416)
            anchor = self.object('C2dPositionObject')
            self.put(container+0x15F8, '<Q', anchor)
            self.folder_anchors[name] = anchor
        self.item = self.object('CTechnologyTreeTechItem')
        self.put(self.item+0xC8, '<Q', self.model)
        self.put(self.item+0x30, '<Q', view)
        self.classes[struct.unpack('<Q', self.read(self.definition, 8))[0]-self.base] = '.?AVCTechnologyTemplate@@'
        container = self.research_container = self.object('CContainerWindow')
        self.put(self.item+0x10, '<Q', container)
        self.put(container+0x98, '<Q', self.root)
        self.put(container+0x80, '<Q', self.item+24)
        self.put(container+0xA5, '<B', 8)
        outer = self.research_outer = self.object('C2dPositionObject'); self.put(container+0x15F0, '<Q', outer)
        self.put(outer+0x118, '<Q', self.folder_anchors['industry_folder'])
        self.put(outer+0x90, '<16f', *matrix(1, 235, -315))
        background = self.research_background = self.object('CButtonDrag')
        definition = self.object('CButtonType'); self.string(definition+0x28, 'Background')
        self.put(background+0x60, '<Q', definition)
        self.put(background+0x50, '<Q', self.item+24); self.put(background+0x68, '<Q', self.root)
        self.put(background+0x110, '<Q', outer)
        sprite = self.object('CSprite'); self.put(background+0x118, '<Q', sprite)
        self.put(sprite+0x118, '<Q', outer); self.put(sprite+0x124, '<f', 1)
        self.put(sprite+0x128, '<2f', 0, 0)
        texture = self.object('CSpriteType'); self.put(sprite+0x180, '<Q', texture)
        self.put(texture+0x90, '<ii', 1, 1); self.put(texture+0xA4, '<ii', 72, 72)
        self.array(container+0x10D8, [background])
        self.anchor = self.object('C2dPositionObject'); self.put(container+0x15F8, '<Q', self.anchor)
        self.put(self.anchor+0x90, '<16f', *matrix(1, 235, -315))
        icon = self.icon = self.object('CIcon')
        definition = self.object('CIconType'); self.string(definition+0x28, 'Icon')
        self.put(icon+0x60, '<Q', definition)
        self.put(icon+0x50, '<Q', self.item+24); self.put(icon+0x68, '<Q', self.root)
        self.put(icon+0xF0, '<Q', self.anchor)
        sprite = self.research_sprite = self.object('CSprite'); self.put(icon+0xE8, '<Q', sprite)
        self.put(sprite+0x118, '<Q', self.anchor)
        self.put(sprite+0x124, '<f', 1); self.put(sprite+0x128, '<2f', 5, 8)
        self.put(sprite+0x90, '<16f', *matrix(1, -5000, 5000))
        texture = self.object('CSpriteType'); self.put(sprite+0x180, '<Q', texture)
        self.put(texture+0x90, '<ii', 1, 1); self.put(texture+0xA4, '<ii', 58, 55)
        self.array(container+0x1150, [icon])
        self.bindings['research']['construction4'] = [self.item]
        self.research_tabs()

    def research_tabs(self):
        owner = self.bindings['views']['CCountryTechTreeView']
        container = self.object('CContainerWindow'); self.bindings['tabs_container'] = container
        self.put(container+0x98, '<Q', self.root); self.put(container+0x80, '<Q', owner+40)
        parent = self.object('C2dPositionObject'); self.put(container+0x15F8, '<Q', parent)
        self.put(parent+0x90, '<16f', *matrix(1, -3, -85))
        buttons = []
        for name, visible, x in [('industry_folder_tab', True, 734), ('electronics_folder_tab', True, 645),
                                  ('unknown_folder_tab', True, 900), ('industry_folder_tab', False, 734)]:
            button = self.object('CButtonStandard'); buttons.append(button)
            definition = self.object('CButtonType'); self.string(definition+0x28, name)
            self.put(button+0x60, '<Q', definition)
            self.put(button+0x50, '<Q', owner+40); self.put(button+0x68, '<Q', self.root)
            self.put(button+0x75, '<B', 8 if visible else 0x10)
            self.put(button+0x1B0, '<Q', parent)
            sprite = self.object('CSprite'); self.put(button+0xF0, '<Q', sprite)
            self.put(sprite+0x118, '<Q', parent); self.put(sprite+0x124, '<f', 1)
            self.put(sprite+0x128, '<2f', x, 0)
            texture = self.object('CSpriteType'); self.put(sprite+0x180, '<Q', texture)
            self.put(texture+0x90, '<ii', 1, 1); self.put(texture+0xA4, '<ii', 91, 61)
        self.array(container+0x1120, buttons)

    def visible_focus(self):
        self.put(self.containers['focus']+0xA5, '<B', 8)
        view = self.bindings['views']['CNationalFocusView']
        for offset in (0x20E8, 0x20F0):
            self.put(view+offset, '<Q', self.tree)
        for offset in (0x2100, 0x2108):
            self.put(view+offset, '<Q', self.country)
        item = self.object('CNationalFocusTreeItem')
        self.put(item+0x20, '<Q', self.focus)
        container = self.object('CContainerWindow')
        self.put(item+0x10, '<Q', container)
        self.put(container+0x98, '<Q', self.root)
        self.put(container+0x80, '<Q', item+24)
        self.put(container+0xA5, '<B', 8)
        self.anchor = self.object('C2dPositionObject')
        self.put(container+0x15F8, '<Q', self.anchor)
        self.put(self.anchor+0x90, '<16f', *matrix(1, 1640, -188))
        self.sprites, buttons = [], []
        for name, position, size in [('bg', [5, 40], [164, 50]), ('symbol', [39, -22], [97, 85])]:
            button = self.object('CButtonStandard'); buttons.append(button)
            definition = self.object('CButtonType')
            self.string(definition+0x28, name)
            self.put(button+0x60, '<Q', definition)
            self.put(button+0x50, '<Q', item+24)
            self.put(button+0x68, '<Q', self.root)
            self.put(button+0x1B0, '<Q', self.anchor)
            sprite = self.object('CSprite'); self.sprites.append(sprite)
            self.put(button+0xF0, '<Q', sprite)
            self.put(sprite+0x118, '<Q', self.anchor)
            self.put(sprite+0x124, '<f', 1.)
            self.put(sprite+0x128, '<2f', *position)
            # Rendering matrix is intentionally stale/unusable. Only the
            # independently current GUI anchor is the admitted coordinate.
            self.put(sprite+0x90, '<16f', *matrix(1, -5000, 5000))
            texture = self.object('CSpriteType')
            self.put(sprite+0x180, '<Q', texture)
            self.put(texture+0x90, '<ii', 1, 1)
            self.put(texture+0xA4, '<ii', *size)
        self.array(container+0x1120, buttons)
        self.bindings['focus']['ROM_test'] = [item]


class FocusUiGeometryTests(unittest.TestCase):
    def test_real_sprite_origin_uses_bottom_left_and_resolution(self):
        got = sprite_geometry(matrix(x=365, y=442), [164, 50], [2560, 1440], [0, 78, 2560, 1362])
        self.assertEqual(got['rect'], [1645., 228., 164., 50.])

    def test_zoom_and_pan_are_both_applied_without_text(self):
        got = sprite_geometry(matrix(.5, 100, -80), [164, 50], [2560, 1440], [0, 0, 2560, 1440])
        self.assertEqual(got['rect'], [1380., 775., 82., 25.])

    def test_live_parent_and_resolved_button_position_are_composed(self):
        got = anchored_sprite_geometry(matrix(.7918363213539124, 741.764404296875, -280.530517578125),
                                       [37., -24.], [100, 88], 1., [0, 78, 2560, 1362])
        self.assertAlmostEqual(got['rect'][0], 771.0623481869698)
        self.assertAlmostEqual(got['rect'][1], 261.5264458656311)
        self.assertAlmostEqual(got['rect'][2], 79.18363213539124)

    def test_live_parent_pan_moves_existing_child_without_render_cache(self):
        a = anchored_sprite_geometry(matrix(.5, 100, -80), [5, 40], [164, 50], 1, [0, 0, 2560, 1440])
        b = anchored_sprite_geometry(matrix(.5, 500, -180), [5, 40], [164, 50], 1, [0, 0, 2560, 1440])
        self.assertEqual(a['rect'], [102.5, 100., 82., 25.])
        self.assertEqual(b['rect'], [502.5, 200., 82., 25.])

    def test_partial_clipping_preserves_real_rect(self):
        got = sprite_geometry(matrix(1, -1290, 690), [50, 50], [2560, 1440], [0, 0, 2560, 1440])
        self.assertEqual(got['rect'], [-10., -20., 50., 50.])
        self.assertEqual(got['clip'], [0, 0, 40., 30.])

    def test_offscreen_is_explicitly_hidden(self):
        self.assertFalse(sprite_geometry(matrix(x=-5000), [50, 50], [2560, 1440], [0, 0, 2560, 1440])['visible'])

    def test_invalid_scale_rotation_and_nan_fail_closed(self):
        for value in (0., -1., math.nan):
            with self.subTest(value=value), self.assertRaises(LiveReadError):
                sprite_geometry(matrix(value), [50, 50], [2560, 1440], [0, 0, 2560, 1440])
        rotated = matrix(); rotated[1] = .2
        with self.assertRaises(LiveReadError):
            sprite_geometry(rotated, [50, 50], [2560, 1440], [0, 0, 2560, 1440])

    def test_container_geometry_keeps_zoomed_size(self):
        position, container = bytearray(0xD0), bytearray(0x15B4)
        struct.pack_into('<16f', position, 0x90, *matrix(.5, 10, -20))
        struct.pack_into('<ii', container, 0x15AC, 165, 128)
        self.assertEqual(geometry(position, container, [0, 0, 2560, 1440])['rect'], [10., 20., 82.5, 64.])


class FocusUiSnapshotTests(unittest.TestCase):
    def test_native_research_painted_icon_is_not_its_container(self):
        f = UiFixture(); f.visible_research()
        got = FixtureReader(process=f).snapshot(technology_ids=['construction4'])
        self.assertEqual(got['active_panel'], 'research')
        self.assertEqual(got['panels']['research']['folder'], 'industry_folder')
        node = got['nodes'][0]
        self.assertEqual(node['rect'], [235., 315., 72., 72.])
        self.assertEqual(node['parts']['icon'], [240., 323., 58., 55.])
        self.assertEqual(node['folder'], 'industry_folder')
        self.assertTrue(node['visible'])

    def test_tabs_use_native_folder_membership_and_dlc_visibility(self):
        f = UiFixture(); f.visible_research(); f.research_tabs()
        tabs = FixtureReader(process=f).snapshot()['tabs']
        self.assertEqual(len(tabs), 3)  # Unknown native GUI name is not a folder ID.
        electronics = next(t for t in tabs if t['identity']=='electronics_folder')
        self.assertEqual(electronics['rect'], [642., 85., 91., 61.])
        self.assertTrue(electronics['visible'])
        self.assertEqual([t['visible'] for t in tabs if t['identity']=='industry_folder'], [True, False])

    def test_folder_switch_during_sampling_is_not_published(self):
        f = UiFixture(); f.visible_research(); read = f.read; counts = [0]
        address = f.bindings['folders']['industry_folder']+0xA4
        def changing(pointer, size):
            if pointer == address:
                counts[0] += 1
                if counts[0] > 1:
                    return b'\0\0'
            return read(pointer, size)
        f.read = changing
        reader=FixtureReader(process=f);reader._cache=('previous',)
        with self.assertRaisesRegex(LiveReadError, 'incoerente|[Cc]artella'):
            reader.snapshot(technology_ids=['construction4'])
        self.assertIsNone(reader._cache)  # Retry found a structurally ambiguous folder.

    def test_category_animation_bits_do_not_invalidate_unchanged_visibility(self):
        f=UiFixture();f.visible_research();original=f.read
        container=f.bindings['tabs_container']
        array=struct.unpack('<Q',f.read(container+0x1120,8))[0]
        button=struct.unpack('<Q',f.read(array,8))[0]
        reads=[0]
        def animated(address,size):
            if address==button+0x75 and size==1:
                reads[0]+=1
                return bytes([0x48 if reads[0]%2 else 0xE8])
            return original(address,size)
        f.read=animated
        result=FixtureReader(process=f).snapshot(technology_ids=['construction4'])
        tab=next(t for t in result['tabs'] if t['identity']=='industry_folder' and t['visible'])
        self.assertEqual(tab['rect'],[731.,85.,91.,61.])
        self.assertEqual(result['diagnostics']['retry_count'],0)

    def test_category_visibility_bit_change_is_still_rejected(self):
        f=UiFixture();f.visible_research();original=f.read
        array=struct.unpack('<Q',f.read(f.bindings['tabs_container']+0x1120,8))[0]
        button=struct.unpack('<Q',f.read(array,8))[0];reads=[0]
        def changing(address,size):
            if address==button+0x75 and size==1:
                reads[0]+=1
                return bytes([0x48 if reads[0]%2 else 0x40])
            return original(address,size)
        f.read=changing
        with self.assertRaisesRegex(LiveReadError,'incoerente'):
            FixtureReader(process=f).snapshot(technology_ids=['construction4'])
        self.assertEqual(reads[0],4)

    def test_research_other_folder_stays_hidden_and_keeps_true_folder(self):
        f = UiFixture(); f.visible_research()
        f.put(f.bindings['folders']['industry_folder']+0xA5, '<B', 0)
        f.put(f.bindings['folders']['electronics_folder']+0xA5, '<B', 8)
        got = FixtureReader(process=f).snapshot(technology_ids=['construction4'])
        self.assertEqual(got['panels']['research']['folder'], 'electronics_folder')
        self.assertEqual(got['nodes'][0]['folder'], 'industry_folder')
        self.assertFalse(got['nodes'][0]['visible'])

    def test_research_foreign_technology_status_rejected(self):
        f = UiFixture(); f.visible_research()
        f.put(f.model+0x158, '<Q', f.object('CTechnologyStatus'))
        with self.assertRaisesRegex(LiveReadError, 'altra nazione'):
            FixtureReader(process=f).snapshot(technology_ids=['construction4'])

    def test_research_current_parent_controls_scroll_and_zoom(self):
        f = UiFixture(); f.visible_research(); reader = FixtureReader(process=f)
        reader.snapshot(technology_ids=['construction4'])
        f.put(f.anchor+0x90, '<16f', *matrix(.5, 100, -200))
        f.put(f.research_outer+0x90, '<16f', *matrix(.5, 100, -200))
        self.assertEqual(reader.snapshot(technology_ids=['construction4'])['nodes'][0]['rect'], [100., 200., 36., 36.])

    def test_research_disconnected_folder_ancestry_is_unknown(self):
        f = UiFixture(); f.visible_research()
        outer = struct.unpack('<Q', f.read(f.research_container+0x15F0, 8))[0]
        f.put(outer+0x118, '<Q', 0)
        with self.assertRaisesRegex(LiveReadError, 'Cartella'):
            FixtureReader(process=f).snapshot(technology_ids=['construction4'])

    def test_research_alternate_view_and_ambiguous_folder_rejected(self):
        f = UiFixture(); f.visible_research()
        f.put(f.item+0x30, '<Q', f.object('CCountryTechTreeView'))
        with self.assertRaisesRegex(LiveReadError, 'altro albero'):
            FixtureReader(process=f).snapshot(technology_ids=['construction4'])
        f = UiFixture(); f.visible_research()
        f.put(f.bindings['folders']['electronics_folder']+0xA5, '<B', 8)
        with self.assertRaisesRegex(LiveReadError, 'ambigua'):
            FixtureReader(process=f).snapshot()

    def test_research_tree_context_reuse_and_missing_tabs_are_rejected(self):
        f = UiFixture(); f.visible_research()
        definition = struct.unpack('<Q', f.read(f.containers['research']+0x90, 8))[0]
        f.string(definition+0x28, 'countrydoctrinetreeview')
        with self.assertRaisesRegex(LiveReadError, 'Contesto'):
            FixtureReader(process=f).snapshot()
        f = UiFixture(); f.visible_research(); f.bindings['tabs_container'] = None
        with self.assertRaisesRegex(LiveReadError, 'Schede native'):
            FixtureReader(process=f).snapshot()

    def test_background_required_instead_of_container_fallback(self):
        f = UiFixture(); f.visible_research()
        f.array(f.research_container+0x10D8, [])
        with self.assertRaisesRegex(LiveReadError, 'Sfondo'):
            FixtureReader(process=f).snapshot(technology_ids=['construction4'])

    def test_foreign_or_duplicate_research_background_fails_closed(self):
        f = UiFixture(); f.visible_research()
        f.put(f.research_background+0x50, '<Q', f.item+40)
        with self.assertRaisesRegex(LiveReadError, 'proprietario'):
            FixtureReader(process=f).snapshot(technology_ids=['construction4'])
        f = UiFixture(); f.visible_research()
        f.array(f.research_container+0x10D8, [f.research_background, f.research_background])
        with self.assertRaisesRegex(LiveReadError, 'ambiguo'):
            FixtureReader(process=f).snapshot(technology_ids=['construction4'])
    def test_focus_geometry_ignores_stale_render_matrix(self):
        f = UiFixture(); f.visible_focus()
        node = FixtureReader(process=f).snapshot(focus_ids=['ROM_test'])['nodes'][0]
        self.assertEqual(node['rect'], [1645., 166., 164., 112.])

    def test_existing_focus_follows_pan_and_zoom_of_its_own_parent(self):
        f = UiFixture(); f.visible_focus(); reader = FixtureReader(process=f)
        reader.snapshot(focus_ids=['ROM_test'])
        f.put(f.anchor+0x90, '<16f', *matrix(.5, 100, -200))
        node = reader.snapshot(focus_ids=['ROM_test'])['nodes'][0]
        self.assertEqual(node['rect'], [102.5, 189., 82., 56.])

    def test_shared_or_foreign_sprite_parent_is_rejected(self):
        f = UiFixture(); f.visible_focus()
        f.put(f.sprites[0]+0x118, '<Q', f.object('C2dPositionObject'))
        with self.assertRaisesRegex(LiveReadError, 'Ancoraggio'):
            FixtureReader(process=f).snapshot(focus_ids=['ROM_test'])

    def test_hidden_panels_and_null_folder_are_authoritative(self):
        f = UiFixture()
        got = FixtureReader(process=f).snapshot(focus_ids=['ROM_test'])
        self.assertEqual(got['country'], 'ROM')
        self.assertEqual(got['scope'], 'romanian_focus')
        self.assertEqual(got['nodes'], [])
        self.assertIsNone(got['active_panel'])
        self.assertIsNone(got['panels']['research']['folder'])

    def test_live_primary_country_identity_is_not_relaxed(self):
        f = UiFixture(); f.put(f.world+0x520, '<i', 0)
        reader = FixtureReader(process=f); reader._cache = ('previous',)
        with self.assertRaises(LiveReadError):
            reader.snapshot()
        self.assertIsNone(reader._cache)

    def test_gui_owner_mismatch_rejects_and_invalidates_bindings(self):
        f = UiFixture(); f.put(f.containers['focus']+0x80, '<Q', 0x12345)
        reader = FixtureReader(process=f); reader._cache = ('previous',)
        with self.assertRaisesRegex(LiveReadError, 'Proprietario'):
            reader.snapshot()
        self.assertIsNone(reader._cache)

    def test_panels_with_different_resolutions_are_rejected(self):
        f = UiFixture()
        pos = struct.unpack('<Q', f.read(f.containers['research']+0x15F0, 8))[0]
        f.put(pos+0x130, '<ii', 1920, 1080)
        with self.assertRaisesRegex(LiveReadError, 'risoluzioni'):
            FixtureReader(process=f).snapshot()

    def test_torn_geometry_is_not_published(self):
        f = UiFixture(); original = f.read; reads = [0]
        address = f.containers['focus']+0xA4
        def read(pointer, size):
            if pointer == address:
                reads[0] += 1
                if reads[0] % 2 == 0:
                    return b'\0\x08'
            return original(pointer, size)
        f.read = read
        reader=FixtureReader(process=f);cached=('candidates',);reader._cache=cached
        with self.assertRaisesRegex(LiveReadError, 'incoerente'):
            reader.snapshot()
        self.assertEqual(reads[0],4)  # Two attempts, each with a fresh verification.
        self.assertIs(reader._cache,cached)

    def test_moving_geometry_retries_fresh_sample_without_repeated_registry_discovery(self):
        f = UiFixture(); f.visible_focus(); original = f.read; reads = [0]
        address = f.anchor+0x90
        class CountingReader(FixtureReader):
            def _discover(self,sample,root):
                self.discoveries=getattr(self,'discoveries',0)+1
                return super()._discover(sample,root)
        reader=CountingReader(process=f)
        cached=(f.root,('allocation',),f.bindings);reader._cache=cached
        def read(pointer,size):
            if pointer==address:
                reads[0]+=1
                if reads[0]==2:f.put(address,'<16f',*matrix(.5,100,-200))
            return original(pointer,size)
        f.read=read
        got=reader.snapshot(focus_ids=['ROM_test'])
        self.assertEqual(got['nodes'][0]['rect'],[102.5,189.,82.,56.])
        self.assertEqual(got['diagnostics']['retry_count'],1)
        self.assertIs(reader._cache,cached)
        self.assertEqual(reader.discoveries,2)

    def test_visible_research_without_folder_is_unknown_not_hidden(self):
        f = UiFixture(); f.put(f.containers['research']+0xA5, '<B', 8)
        f.research_tabs()
        with self.assertRaisesRegex(LiveReadError, '[Cc]artella'):
            FixtureReader(process=f).snapshot()

    def test_visible_foreign_focus_frontend_is_not_bound_by_same_id(self):
        f = UiFixture(); f.put(f.containers['focus']+0xA5, '<B', 8)
        owner = f.bindings['views']['CNationalFocusView']
        for offset in (0x20E8, 0x20F0):
            f.put(owner+offset, '<Q', f.tree)
        for offset in (0x20F8, 0x2100, 0x2108):
            f.put(owner+offset, '<Q', f.country)
        f.put(owner+0x2100, '<Q', f.object('CCountry'))
        with self.assertRaisesRegex(LiveReadError, 'altra nazione'):
            FixtureReader(process=f).snapshot(focus_ids=['ROM_test'])

    def test_cold_discovery_registry_mutation_is_not_gameplay_evidence(self):
        f = UiFixture()
        class MutatingDiscovery(FixtureReader):
            def _discover(self, sample, root):
                sample.read(root+0x108, 16)
                # An unrelated tooltip can grow/reuse the candidate registry
                # while discovery runs. Final bound widgets remain identical.
                f.put(root+0x108, '<Qii', 0x300000000, 256, 32)
                return f.bindings
        self.assertEqual(MutatingDiscovery(process=f).snapshot()['country'], 'ROM')

    def test_unrelated_registry_occupancy_does_not_rescan_cached_bindings(self):
        f = UiFixture()
        for offset in (0x108, 0x120):
            f.put(f.root+offset, '<Qii', 0x300000000, 128, 10)
        reader = FixtureReader(process=f)
        reader._cache = (f.root, ((0x300000000, 128), (0x300000000, 128)), f.bindings)
        f.put(f.root+0x120, '<Qii', 0x300000000, 128, 11)
        # Cached bindings are reused; any actual observation is still sampled
        # again by snapshot, independently of this transient occupancy count.
        self.assertIs(UiReader._discover(reader, NativeSample(f), f.root), f.bindings)

    def test_nullable_focus_frontend_context_keeps_persistent_country_checks(self):
        f = UiFixture(); f.put(f.containers['focus']+0xA5, '<B', 8)
        owner = f.bindings['views']['CNationalFocusView']
        for offset in (0x20E8, 0x20F0):
            f.put(owner+offset, '<Q', f.tree)
        for offset in (0x2100, 0x2108):
            f.put(owner+offset, '<Q', f.country)
        self.assertEqual(FixtureReader(process=f).snapshot()['active_panel'], 'focus')

    def test_missing_target_refreshes_once_per_request_context(self):
        f = UiFixture(); f.put(f.containers['focus']+0xA5, '<B', 8)
        owner = f.bindings['views']['CNationalFocusView']
        for offset in (0x20E8, 0x20F0):
            f.put(owner+offset, '<Q', f.tree)
        for offset in (0x2100, 0x2108):
            f.put(owner+offset, '<Q', f.country)
        class CountingReader(FixtureReader):
            def _discover(self, sample, root):
                self.calls = getattr(self, 'calls', 0)+1
                return f.bindings
        reader = CountingReader(process=f)
        self.assertEqual(reader.snapshot(focus_ids=['absent'])['nodes'], [])
        self.assertEqual(reader.calls, 2)  # Initial lookup, then one refresh.
        reader.snapshot(focus_ids=['absent'])
        self.assertEqual(reader.calls, 3)  # No additional recursive refresh.
        reader.snapshot(focus_ids=['another_absent'])
        self.assertEqual(reader.calls, 5)  # A new target has one fresh attempt.


if __name__ == '__main__':
    unittest.main()
