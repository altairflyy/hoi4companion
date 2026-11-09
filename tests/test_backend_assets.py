"""Native sprite identities preserve texture precedence, frames and path guards."""
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
import struct
from PIL import Image
from hoi4companion.backend_core import _request
from hoi4companion._sprites_reference import SpriteLibrary

class BackendAssetsTests(unittest.TestCase):
    def test_native_png_decoding_cropping_and_dx10_bgra(self):
        with TemporaryDirectory() as folder:
            root=Path(folder)/'game';(root/'interface').mkdir(parents=True);(root/'gfx').mkdir()
            (root/'interface/icons.gfx').write_text('spriteTypes={spriteType={name=GFX_test texturefile="gfx/icon.dds" noOfFrames=2}}')
            for mode in ('RGBA','DX10'):
                path=root/'gfx/icon.dds'
                if mode=='RGBA':
                    with Image.new('RGBA',(8,4),(240,15,5,128)) as image:image.save(path,format='DDS')
                    expected=(240,15,5,128);size=(4,4)
                else:
                    header=bytearray(148);header[:4]=b'DDS ';struct.pack_into('<I',header,4,124)
                    struct.pack_into('<III',header,12,1,2,8);header[84:88]=b'DX10';struct.pack_into('<IIII',header,128,91,3,0,1)
                    path.write_bytes(header+bytes([3,2,1,128,6,5,4,255]));expected=(4,5,6,255);size=(1,1)
                original=path.read_bytes()
                handle=_request(0,'assets_open',game=str(root),roots=[],cache=str(Path(folder)/'cache'))
                try:
                    sprite=_request(0,'assets_resolve',handle=handle,name='GFX_test',frame=2)['sprite'];token=sprite['url'].split('/')[-1][:-4]
                    result=_request(0,'assets_png',handle=handle,token=token)
                    with Image.open(result) as image:self.assertEqual((image.size,image.getpixel((0,0))),(size,expected))
                    self.assertIsNone(_request(0,'assets_png',handle=handle,token='../secret'))
                    self.assertEqual(path.read_bytes(),original)
                finally:_request(0,'assets_release',handle=handle)

    def test_sprite_identity_override_frame_bounds_and_replace_path(self):
        with TemporaryDirectory() as folder:
            game,mod=Path(folder)/'game',Path(folder)/'mod'
            (game/'interface').mkdir(parents=True);(mod/'gfx').mkdir(parents=True)
            (game/'interface/icons.gfx').write_text('spriteTypes={spriteType={name=GFX_test texturefile="gfx/icon.png" noOfFrames=2} spriteType={name=GFX_bad texturefile="../secret.dds"}}')
            with Image.new('RGBA',(8,4),(1,2,3,4)) as image:image.save(mod/'gfx/icon.dds',format='DDS')
            reference=SpriteLibrary(game,[mod],Path(folder)/'cache')
            handle=_request(0,'assets_open',game=str(game),roots=[str(mod)])
            self.addCleanup(lambda:_request(0,'assets_release',handle=handle))
            for name,frame in [('GFX_test',1),('GFX_test',2),('GFX_test',0),('GFX_test',3),('GFX_bad',1),('unknown',1)]:
                native=_request(0,'assets_resolve',handle=handle,name=name,frame=frame)
                self.assertEqual(native['sprite'],reference.resolve(name,frame))
                if native['sprite']:self.assertEqual(native['asset']['path'],str((mod/'gfx/icon.dds').resolve()))

    def test_png_tga_bmp_and_compressed_dds_keep_rgba_pixels(self):
        with TemporaryDirectory() as folder:
            game=Path(folder)/'game';(game/'interface').mkdir(parents=True);(game/'gfx').mkdir()
            for index,(format,options) in enumerate([('PNG',{}),('TGA',{}),('BMP',{}),('DDS',{'pixel_format':'DXT1'}),('DDS',{'pixel_format':'DXT5'})]):
                texture=game/'gfx'/('icon.'+format.lower())
                with Image.new('RGBA',(8,8),(40,80,120,128)) as image:image.save(texture,format=format,**options)
                (game/'interface/icon.gfx').write_text('spriteTypes={spriteType={name=GFX_test texturefile="gfx/'+texture.name+'"}}')
                handle=_request(0,'assets_open',game=str(game),roots=[],cache=str(Path(folder)/('cache'+str(index))))
                try:
                    sprite=_request(0,'assets_resolve',handle=handle,name='GFX_test',frame=1)['sprite'];token=sprite['url'].split('/')[-1][:-4]
                    converted=_request(0,'assets_png',handle=handle,token=token)
                    self.assertIsNotNone(converted,format)
                    with Image.open(texture) as reference,Image.open(converted) as actual:
                        self.assertEqual(actual.tobytes(),reference.convert('RGBA').tobytes(),format)
                finally:_request(0,'assets_release',handle=handle)

    def test_cache_cannot_write_into_game_even_with_parent_components(self):
        with TemporaryDirectory() as folder:
            game=Path(folder)/'game';(game/'interface').mkdir(parents=True);(game/'gfx').mkdir()
            (game/'interface/icon.gfx').write_text('spriteTypes={spriteType={name=GFX_test texturefile="gfx/icon.png"}}')
            with Image.new('RGBA',(4,4)) as image:image.save(game/'gfx/icon.png')
            cache=game/'unused/../cache'
            handle=_request(0,'assets_open',game=str(game),roots=[],cache=str(cache))
            try:
                sprite=_request(0,'assets_resolve',handle=handle,name='GFX_test',frame=1)['sprite'];token=sprite['url'].split('/')[-1][:-4]
                with self.assertRaisesRegex(ValueError,'outside game'):_request(0,'assets_png',handle=handle,token=token)
                self.assertFalse((game/'unused').exists());self.assertFalse((game/'cache').exists())
            finally:_request(0,'assets_release',handle=handle)
