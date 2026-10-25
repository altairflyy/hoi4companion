import struct
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from hoi4companion.focus_ui_native import UiReader
from hoi4companion.live import LiveReadError
from test_focus_ui_native import UiFixture, FixtureReader


def sprite_reader(kind='CFrameAnimatedSprite',mdisp=0,pdisp=-1,displacement=0,base='CSprite'):
    data=bytearray(4096);image_base=0x140000000;process_base=0x180000000
    struct.pack_into('<Q',data,0x7F8,image_base+0x100)
    struct.pack_into('<6I',data,0x100,1,displacement,0,0x200,0x300,0x100)
    struct.pack_into('<4I',data,0x300,0,0,2,0x400)
    struct.pack_into('<2I',data,0x400,0x500,0x540)
    struct.pack_into('<IIiiiII',data,0x500,0x200,1,0,-1,0,0x40,0x300)
    struct.pack_into('<IIiiiII',data,0x540,0x600,0,mdisp,pdisp,0,0x40,0x300)
    for offset,name in ((0x210,kind),(0x610,base)):
        encoded=('.?AV'+name+'@@').encode()+b'\0';data[offset:offset+len(encoded)]=encoded
    def read(offset,length):
        if not 0<=offset<=len(data)-length:raise LiveReadError('RTTI fuori immagine')
        return bytes(data[offset:offset+length])
    reader=UiReader.__new__(UiReader)
    reader.process=SimpleNamespace(base=process_base)
    reader.profile=SimpleNamespace(image_base=image_base,bytes=read)
    return reader,SimpleNamespace(number=lambda _:process_base+0x800)


class SpriteInheritanceTests(unittest.TestCase):
    def test_animated_sprite_requires_verified_nonvirtual_primary_base(self):
        reader,sample=sprite_reader()
        self.assertEqual(reader._sprite(sample,1234),1234)
        for changes in ({'mdisp':8},{'pdisp':0},{'displacement':8},{'kind':'CUnrelatedSprite'}):
            reader,sample=sprite_reader(**changes)
            with self.assertRaises(LiveReadError):reader._sprite(sample,1234)

    def test_animated_sprite_definition_has_the_same_strict_base_requirement(self):
        reader,sample=sprite_reader(kind='CFrameAnimatedSpriteType',base='CSpriteType')
        self.assertEqual(reader._sprite(sample,1234,'CSpriteType'),1234)
        reader,sample=sprite_reader(kind='CFrameAnimatedSpriteType',base='CSpriteType',mdisp=8)
        with self.assertRaises(LiveReadError):reader._sprite(sample,1234,'CSpriteType')

    def test_active_research_background_preserves_geometry_and_owner_checks(self):
        fixture=UiFixture();fixture.visible_research()
        sprite=struct.unpack('<Q',fixture.read(fixture.research_background+0x118,8))[0]
        animated=fixture.object('CFrameAnimatedSprite')
        vtable=struct.unpack('<Q',fixture.read(animated,8))[0]
        fixture.put(sprite,'<Q',vtable)
        texture=struct.unpack('<Q',fixture.read(sprite+0x180,8))[0]
        animated_type=fixture.object('CFrameAnimatedSpriteType')
        texture_vtable=struct.unpack('<Q',fixture.read(animated_type,8))[0]
        fixture.put(texture,'<Q',texture_vtable)
        reader=FixtureReader(process=fixture)
        with patch.object(reader,'_primary_base',return_value=True) as verify:
            result=reader.snapshot(technology_ids=['construction4'])
            self.assertEqual(result['nodes'][0]['rect'],[235.,315.,72.,72.])
            verify.assert_any_call(vtable,'CSprite');verify.assert_any_call(texture_vtable,'CSpriteType')
            fixture.put(sprite+0x118,'<Q',fixture.object('C2dPositionObject'))
            with self.assertRaisesRegex(LiveReadError,'Ancoraggio'):reader.snapshot(technology_ids=['construction4'])
