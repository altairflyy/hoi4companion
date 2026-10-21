from pathlib import Path
import tempfile
import unittest

from hoi4companion.dlc import DlcInventory
from hoi4companion.technologies import load_technologies


class TechnologyLayoutTest(unittest.TestCase):
    def test_artillery_headings_follow_centered_parent_and_minimum_viewport_width(self):
        from hoi4companion.clausewitz import parse
        from hoi4companion.technology_layout import _walk
        gui = parse('containerWindowType={size={width=1515 height=1390 min={width=100%%}} containerWindowType={position={x=35 y=50} size={width=600 height=80%} orientation=center_up origo=center_up instantTextBoxType={name=artillery_subtitle_artillery position={x=105 y=55}}}}')
        title = [(x,y) for kind, child, x, y in _walk(gui) if child.scalar('name') == 'artillery_subtitle_artillery']
        self.assertEqual(title, [(1101.0,105.0)])

    def test_shipped_heavy_heading_typo_has_narrow_screenshot_based_repair(self):
        from hoi4companion.clausewitz import parse
        from hoi4companion.technology_layout import _walk
        gui = parse('containerWindowType={position={x=750 y=40} instantTextBoxType={name=armour_subtitle_heavy position={x=275k y=50}} instantTextBoxType={name=other position={x=275k y=50}}}')
        titles = [(child.scalar('name'), x, y) for kind, child, x, y in _walk(gui) if kind == 'instanttextboxtype']
        self.assertEqual(titles, [('armour_subtitle_heavy', 1025.0, 90.0)])

    def load(self, source, gui):
        with tempfile.TemporaryDirectory() as tmp:
            file, layout = Path(tmp) / 'tech.txt', Path(tmp) / 'layout.gui'
            file.write_text(source, encoding='utf-8')
            layout.write_text(gui, encoding='utf-8')
            return load_technologies([file], [file], {'TITLE': 'Actual heading'}, DlcInventory(), {}, gui_files=[layout])

    def test_declared_tabs_coordinates_and_branch_subdivision(self):
        catalog = self.load('''technology_folders={test={}}
          technologies={A={folder={name=test position={x=0 y=0}} path={leads_to_tech=B}}
          B={folder={name=test position={x=2 y=4}} path={leads_to_tech=A}}
          C={folder={name=test position={x=1 y=3}}} unmapped={folder={name=test}}}''',
          '''guiTypes={containerWindowType={name=countrytechtreeview
            containerWindowType={name=folder_tabs buttonType={name=test_tab position={x=111 y=0} quadTextureSprite=GFX_tab}}
            containerWindowType={name=test position={x=0 y=47}
             containerWindowType={name=decoration position={x=5 y=10}
               instantTextBoxType={name=test_subtitle text=TITLE position={x=30 y=40}}}
             gridboxtype={name=A_tree position={x=100 y=200} slotsize={width=70 height=60} format=LEFT}
             gridboxtype={name=C_tree position={x=800 y=50} slotsize={width=70 height=60} format=UP}}}}''')
        folder = catalog['folders']['test']
        self.assertEqual(folder['tab']['order'], 111)
        self.assertEqual(folder['tab']['sprite_name'], 'GFX_tab')
        self.assertEqual(folder['layout']['mapped'], 3)
        self.assertEqual(folder['layout']['captions'][0]['text'], 'Actual heading')
        self.assertEqual(folder['layout']['captions'][0]['x'], 35)
        self.assertEqual(folder['layout']['captions'][0]['y'], 50)
        memberships = {key: value['folders'][0] for key, value in catalog['technologies'].items()}
        self.assertEqual((memberships['B']['layout']['x'], memberships['B']['layout']['y']), (380, 290))
        self.assertEqual((memberships['C']['layout']['x'], memberships['C']['layout']['y']), (870, 230))
        self.assertNotIn('layout', memberships['unmapped'])
        self.assertEqual([s['id'] for s in folder['sections']], ['A', 'C'])

    def test_shared_descendant_is_not_assigned_arbitrarily(self):
        c = self.load('''technology_folders={f={}} technologies={
          A={folder={name=f position={x=0 y=0}} path={leads_to_tech=X}}
          B={folder={name=f position={x=0 y=0}} path={leads_to_tech=X}}
          X={folder={name=f position={x=1 y=1}}}}''', '''guiTypes={containerWindowType={name=f
          gridboxtype={name=A_tree slotsize={width=70 height=70} format=UP}
          gridboxtype={name=B_tree slotsize={width=70 height=70} format=UP}}}''')
        self.assertNotIn('layout', c['technologies']['X']['folders'][0])
        self.assertEqual(c['folders']['f']['layout']['mapped'], 2)

    def test_invalid_gui_fallback_is_explicit(self):
        c = self.load('technology_folders={test={}} technologies={A={folder={name=test position={x=0 y=0}}}}',
                      'guiTypes={containerWindowType={name=test gridboxtype={name=A_tree slotsize={width=nan height=70} format=UP}}}')
        self.assertEqual(c['folders']['test']['layout']['mapped'], 0)
        c = self.load('technology_folders={test={}}', 'guiTypes={')
        self.assertTrue(c['warnings'])

    def test_percentage_grid_origin_uses_explicit_reference_viewport(self):
        c = self.load('technology_folders={test={}} technologies={A={folder={name=test position={x=0 y=0}}}}',
                      'guiTypes={containerWindowType={name=test gridboxtype={name=A_tree position={x=10% y=20} slotsize={width=70 height=70} format=UP}}}')
        self.assertEqual(c['folders']['test']['layout']['mapped'], 1)
        self.assertEqual(c['technologies']['A']['folders'][0]['layout']['x'], 256)
        self.assertEqual(c['folders']['test']['layout']['reference_viewport_width'], 2560)

    def test_layout_ignored_edge_does_not_move_shared_naval_armor(self):
        c = self.load('''technology_folders={f={}} technologies={
          A={folder={name=f position={x=0 y=0}} path={leads_to_tech=X}}
          B={folder={name=f position={x=0 y=0}} path={leads_to_tech=X ignore_for_layout=yes}}
          X={folder={name=f position={x=1 y=1}}}}''', '''guiTypes={containerWindowType={name=f
          gridboxtype={name=A_tree slotsize={width=70 height=70} format=UP}
          gridboxtype={name=B_tree slotsize={width=70 height=70} format=UP}}}''')
        self.assertEqual(c['technologies']['X']['folders'][0]['layout']['section_id'], 'A')
        self.assertEqual(c['technologies']['B']['layout_ignored_paths'], ['X'])
        self.assertEqual(c['folders']['f']['layout']['mapped'], 3)

    def test_declared_alignment_and_templates_match_screenshot_landmarks(self):
        # Values are transcribed from installed countrytechtreeview.gui and tech files;
        # screenshot reference is 2560-wide, with content origin approximately (10,60).
        c = self.load('''technology_folders={industry_folder={} infantry_folder={} nsb_armour_folder={}}
          technologies={basic_machine_tools={folder={name=industry_folder position={x=0 y=0}}}
          infantry_weapons={enable_equipments={rifle} folder={name=infantry_folder position={x=0 y=-1}}}
          gwtank_chassis={show_equipment_icon=yes folder={name=nsb_armour_folder position={x=2 y=0}}}}''',
          '''guiTypes={
           containerWindowType={name=techtree_industry_folder_item position={x=0 y=5} size={width=204 height=72}}
           containerWindowType={name=techtree_infantry_folder_item position={x=-56 y=-7} size={width=183 height=84}
             instantTextBoxType={name=Name}}
           containerWindowType={name=techtree_nsb_armour_folder_item position={x=-55 y=0} size={width=183 height=84}
             instantTextBoxType={name=Name}}
           containerWindowType={name=industry_folder
             gridboxtype={name=basic_machine_tools_tree position={x=0 y=172} size={width=520 height=800} slotsize={width=70 height=70} format=UP}}
           containerWindowType={name=infantry_folder
             gridboxtype={name=infantry_weapons_tree position={x=140 y=325} slotsize={width=70 height=70} format=LEFT}}
           containerWindowType={name=nsb_armour_folder
             gridboxtype={name=gwtank_chassis_tree position={x=750 y=40} slotsize={width=70 height=70} format=UPPER_LEFT}}}''')
        expected = {'basic_machine_tools': (261, 213, 72, 72), 'infantry_weapons': (105.5, 325, 183, 84),
                    'gwtank_chassis': (926.5, 82, 183, 84)}
        for key, rect in expected.items():
            p = c['technologies'][key]['folders'][0]['layout']
            self.assertEqual((p['center_x'], p['center_y'], p['width'], p['height']), rect)

    def test_left_grid_cross_axis_centers_inside_declared_height(self):
        c = self.load('technology_folders={test={}} technologies={A={folder={name=test position={x=0 y=2}}}}',
                      'guiTypes={containerWindowType={name=test gridboxtype={name=A_tree position={x=70 y=60} size={width=100 height=140} slotsize={width=70 height=70} format=LEFT}}}')
        p = c['technologies']['A']['folders'][0]['layout']
        self.assertEqual((p['anchor_x'], p['anchor_y']), (210, 95))

    def test_filtered_dlc_root_marks_descendants_as_additional_data(self):
        c = self.load('''technology_folders={test={}} technologies={
          root={allow_branch={has_dlc=Missing} folder={name=test position={x=0 y=0}} path={leads_to_tech=child}}
          child={folder={name=test position={x=0 y=2}}}}''',
          'guiTypes={containerWindowType={name=test gridboxtype={name=root_tree slotsize={width=70 height=70} format=LEFT}}}')
        self.assertNotIn('root', c['technologies'])
        self.assertIn('root', c['technologies']['child']['layout_hidden_reason'])
        self.assertEqual(c['folders']['test']['layout']['hidden'][0]['id'], 'child')


if __name__ == '__main__':
    unittest.main()
