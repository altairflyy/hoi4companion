"""Native picker fixtures prove admission guards, not real-screen alignment."""
import unittest
from hoi4companion.live import LiveReadError
import test_production_ui_native as production_ui_tests
from test_focus_ui_native import matrix


class ProductionControlTests(unittest.TestCase):
    def setUp(self):
        self.fixture=production_ui_tests.ProductionUiTests();self.fixture.setUp()
        self.f=self.fixture.f;self.ui=self.fixture.ui;self.bindings=self.fixture.bindings
        self.window=self.f.object('CProductionEquipmentWindow')
        self.f.put(self.fixture.view+0x2730,'<Q',self.window)
        self.f.put(self.window+0x10F8,'<Q',self.fixture.view)
        self.f.array(self.window+0x10C0,[]);self.f.array(self.window+0x10D8,[])
        self.bindings['production_add']=[{'address':self.button(self.fixture.view,'tank_button','CCountryProductionLineView'),
                                          'owner':self.fixture.view,'name':'tank_button'}]

    def button(self,owner,name,kind):
        f=self.f;address=f.object('CButtonStandard');f.put(address+0x68,'<Q',self.fixture.root)
        observer=f.object(kind);vtable=int.from_bytes(f.read(observer,8),'little')
        f.put(owner+24,'<Q',vtable);f.put(address+0x50,'<Q',owner+24)
        original=self.ui._rtti;self.ui._rtti=lambda vt:(original(vt)[0],24 if vt==vtable else original(vt)[1])
        definition=f.object('CButtonType');f.string(definition+0x28,name);f.put(address+0x60,'<Q',definition)
        parent=f.object('C2dPositionObject');f.put(parent+0x90,'<16f',*matrix(1,550,-250));f.put(address+0x1B0,'<Q',parent)
        sprite=f.object('CSprite');f.put(address+0xF0,'<Q',sprite);f.put(sprite+0x118,'<Q',parent)
        f.put(sprite+0x124,'<f',1);f.put(sprite+0x128,'<2f',0,0)
        texture=f.object('CSpriteType');f.put(sprite+0x180,'<Q',texture)
        f.put(texture+0x90,'<ii',1,1);f.put(texture+0xA4,'<ii',300,70)
        f.put(texture+0x10C,'<i',0);f.put(address+0x75,'<B',8)
        return address

    def open_picker(self):
        f=self.f;c=self.fixture.container_for(self.window+24)
        definition=f.object('CContainerWindowType');f.put(c+0x90,'<Q',definition);f.string(definition+0x28,'production_equipment_window_military')
        viewport=self.fixture.container_for(self.window+24);definition=f.object('CContainerWindowType')
        f.put(viewport+0x90,'<Q',definition);f.string(definition+0x28,'equipments')
        secondary=f.object('CContainerWindow');vt=int.from_bytes(f.read(secondary,8),'little');f.put(viewport+48,'<Q',vt)
        original=self.ui._rtti;self.ui._rtti=lambda v:(original(v)[0],48 if v==vt else original(v)[1])
        f.array(c+0x10F0,[viewport+48]);f.put(viewport+0x1794,'<4i',539,150,1034,1400)
        self.bindings['production_pickers']=[{'owner':self.window,'container':c,'observer_offset':24,'name':'production_equipment_window_military'}]
        self.item=f.object('CProductionInfoEquipmentItem');f.put(self.item+0x11D8,'<Q',self.window)
        variant=f.object('CEquipmentVariant');equipment=f.object('CEquipmentType')
        f.put(self.item+0x1158,'<Q',variant);f.put(variant+0x3F0,'<Q',equipment)
        f.string(variant+0x28,'Correct Model');f.string(equipment+0x18,'tank')
        row=self.fixture.container_for(self.item+24);f.put(self.item+0x10,'<Q',row)
        self.equipment_button=self.button(self.item,'equipment_button','CProductionInfoEquipmentItem')
        f.array(row+0x1120,[self.equipment_button]);f.array(self.window+0x10C0,[self.item])
        self.picker_container=c

    def test_add_buttons_and_exact_picker_entry_preserve_ownership_and_clipping(self):
        self.assertEqual(self.fixture.snapshot()['add'][0]['id'],'tank_button')
        self.open_picker();result=self.fixture.snapshot()
        self.assertTrue(result['picker_open']);self.assertEqual(result['picker_count'],1)
        self.assertEqual(result['equipment'][0]['equipment'],'tank')
        self.assertEqual(result['equipment'][0]['variant_name'],'Correct Model')
        self.assertEqual(result['equipment'][0]['rect'],[550,250,300,70])
        self.f.put(self.equipment_button+0x75,'<B',0)
        self.assertFalse(self.fixture.snapshot()['equipment'][0]['visible'])

    def test_foreign_picker_item_and_changed_button_identity_fail_closed(self):
        self.open_picker();self.f.put(self.item+0x11D8,'<Q',self.window+8)
        with self.assertRaises(LiveReadError):self.fixture.snapshot()
        self.f.put(self.item+0x11D8,'<Q',self.window)
        self.f.put(self.window+0x10F8,'<Q',self.fixture.view+8)
        with self.assertRaises(LiveReadError):self.fixture.snapshot()

    def test_hidden_picker_does_not_expose_items_and_duplicate_members_are_rejected(self):
        self.open_picker();self.f.put(self.picker_container+0xA4,'<BB',0,0)
        self.assertFalse(self.fixture.snapshot()['picker_open']);self.assertEqual(self.fixture.snapshot()['equipment'],[])
        self.f.array(self.window+0x10D8,[self.item])
        with self.assertRaises(LiveReadError):self.fixture.snapshot()
