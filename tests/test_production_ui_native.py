"""Qualified row ownership and geometry, without requiring a running game."""
import unittest

from hoi4companion.focus_ui_native import GuiSample
from hoi4companion.live import MILITARY_HASH,LiveReadError
from hoi4companion.production_ui_native import read
from test_focus_ui_native import FixtureReader,matrix
from test_native_fixture import Fixture


class ProductionUiTests(unittest.TestCase):
    def setUp(self):
        f=self.f=Fixture()
        f.profile.sha256=MILITARY_HASH
        f.profile.gui_layout={'view_container':0x5F8,'container_outer':0x17F0,'container_clip':0x1794}
        f.profile.production_owner_offset=0x2A8
        self.root=f.object('CGameGui')
        self.view=f.object('CCountryProductionLineView')
        self.container=self.container_for(self.view+40)
        f.put(self.view+0x5F8,'<Q',self.container)
        definition=f.object('CContainerWindowType')
        f.put(self.container+0x90,'<Q',definition)
        f.string(definition+0x28,'countryproductionlineview')
        self.status=f.object('CProductionStatus')
        f.put(f.country+0xF68,'<Q',self.status)
        f.put(self.status+0x2A8,'<Q',f.country)
        self.line=f.object('CMilitaryProductionLine')
        f.array(self.status+0x58,[self.line])
        f.put(self.line+0x20,'<Q',self.status)
        variant=f.object('CEquipmentVariant');equipment=f.object('CEquipmentType')
        f.put(self.line+0x88,'<Q',variant);f.put(variant+0x3F0,'<Q',equipment)
        f.string(variant+0x28,'Turan_I');f.string(equipment+0x18,'medium_tank_chassis_1')
        self.item=f.object('CProductionLineMilitaryItem');f.allocate()
        f.put(self.item+0x3878,'<Q',self.line);f.put(self.item+0xB90,'<Q',self.view)
        self.row=self.container_for(self.item+0xB48)
        f.put(self.item+0x10,'<Q',self.row)
        self.position=f.object('C2dPositionObject')
        f.put(self.row+0x17F0,'<Q',self.position)
        f.put(self.position+0x90,'<16f',*matrix(1,8,-372))
        f.put(self.row+0x17AC,'<ii',500,111)
        self.bindings={'production_owners':{self.view:'CCountryProductionLineView',self.item:'CProductionLineMilitaryItem'}}
        self.ui=FixtureReader(process=f)
        # GUI child arrays contain the CContainerWindow secondary base.
        self.viewport=self.container
        secondary=set()
        for name in ('military_tab','production_lines'):
            child=self.container_for(self.view+40)
            definition=f.object('CContainerWindowType')
            f.string(definition+0x28,name);f.put(child+0x90,'<Q',definition)
            base=f.object('CContainerWindow')
            vtable=int.from_bytes(f.read(base,8),'little');secondary.add(vtable)
            f.put(child+48,'<Q',vtable)
            f.array(self.viewport+0x10F0,[child+48]);self.viewport=child
        f.put(self.viewport+0x1794,'<4i',0,367,550,1427)
        original=self.ui._rtti
        # Both distinct secondary vtables must retain their RTTI displacement.
        self.ui._rtti=lambda vt:(original(vt)[0],48 if vt in secondary else 0)

    def container_for(self,observer):
        f=self.f;c=f.object('CContainerWindow')
        f.put(c+0x98,'<Q',self.root);f.put(c+0x80,'<Q',observer)
        f.put(c+0xA4,'<BB',0,8)
        f.put(c+0x1794,'<4i',0,78,600,1440)
        return c

    def snapshot(self):
        s=GuiSample(self.f)
        result=read(self.ui,s,self.root,self.bindings,[2560,1440],self.f.country)
        s.verify()
        return result

    def test_observed_row_identity_and_scroll_clipping(self):
        row=self.snapshot()['lines'][0]
        self.assertEqual((row['line_id'],row['equipment'],row['variant_name']),('1','medium_tank_chassis_1','Turan_I'))
        self.assertEqual(row['rect'],[8,372,500,111])
        self.f.put(self.position+0x90,'<16f',*matrix(1,8,-340))
        self.assertEqual(self.snapshot()['lines'][0]['clip'],[8,367,500,84])
        self.f.put(self.position+0x90,'<16f',*matrix(1,8,-372))
        self.f.put(self.row+0x1794,'<4i',0,400,600,450)
        self.assertEqual(self.snapshot()['lines'][0]['clip'],[8,400,500,50])
        self.f.put(self.position+0x90,'<16f',*matrix(1,8,-1500))
        self.assertFalse(self.snapshot()['lines'][0]['visible'])

    def test_foreign_country_view_line_and_widget_fail_closed(self):
        for address,value in ((self.status+0x2A8,self.f.country),(self.item+0xB90,self.view),
                              (self.line+0x20,self.status),(self.row+0x98,self.root)):
            with self.subTest(address=address):
                self.f.put(address,'<Q',value+8)
                with self.assertRaises(LiveReadError):self.snapshot()
                self.f.put(address,'<Q',value)
        self.f.array(self.status+0x58,[])
        self.assertEqual(self.snapshot()['lines'],[])

    def test_hidden_ambiguous_profile_and_invalid_transform(self):
        self.f.put(self.container+0xA4,'<BB',0,0)
        self.assertFalse(self.snapshot()['visible']);self.assertEqual(self.snapshot()['lines'],[])
        self.f.put(self.container+0xA4,'<BB',0,8)
        self.f.put(self.position+0x90,'<16f',*matrix(float('nan'),8,-372))
        with self.assertRaises(LiveReadError):self.snapshot()
        self.f.profile.sha256='unqualified'
        self.assertEqual(self.snapshot()['status'],'unavailable')
