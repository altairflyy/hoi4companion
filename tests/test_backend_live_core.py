import unittest
import os
import struct
from test_native_fixture import Fixture
from hoi4companion.live import NativeSample,LiveReadError
from hoi4companion.live_core import read_core
from hoi4companion.live_core import read_process_core
from hoi4companion._live_reference import read_native_state as reference


class BackendLiveCoreTests(unittest.TestCase):
    @unittest.skipUnless(os.name=='nt','Windows read-only process API')
    def test_bulk_windows_acquisition_reads_only_owned_fixture_memory(self):
        import ctypes
        from hoi4companion.live import WORLD_RVA
        arena=ctypes.create_string_buffer(64*1024*1024)
        class OwnedFixture(Fixture):
            def allocate(self):
                value=self.base+self.offset;self.offset+=0x2000;return value
        memory=OwnedFixture.__new__(OwnedFixture);memory.base=ctypes.addressof(arena);memory.offset=0x10000
        OwnedFixture.__init__(memory)
        for address,value in memory.memory.items():arena[address-memory.base]=bytes([value])
        pe=bytearray(0x7000)
        def put(offset,fmt,value):struct.pack_into(fmt,pe,offset,value)
        put(0x3c,'<I',0x80);pe[0x80:0x84]=b'PE\0\0';put(0x84,'<H',0x8664);put(0x86,'<H',1)
        put(0x94,'<H',0xf0);put(0x98,'<H',0x20b);put(0xb0,'<Q',0x140000000)
        section=0x80+24+0xf0;put(section+12,'<I',0x800);put(section+16,'<I',0x6000);put(section+20,'<I',0x400)
        raw=lambda rva:rva-0x800+0x400
        for index,(vtable,name) in enumerate(memory.classes.items()):
            locator=0x2000+index*32;descriptor=0x4000+index*0x100
            put(raw(vtable-8),'<Q',0x140000000+locator);put(raw(locator),'<I',1)
            put(raw(locator+12),'<I',descriptor);put(raw(locator+20),'<I',locator)
            encoded=name.encode('ascii');pe[raw(descriptor+16):raw(descriptor+16)+len(encoded)]=encoded
        memory.profile.data=bytes(pe)
        kernel=ctypes.WinDLL('kernel32',use_last_error=True)
        kernel.OpenProcess.argtypes=[ctypes.c_uint32,ctypes.c_bool,ctypes.c_uint32];kernel.OpenProcess.restype=ctypes.c_void_p
        kernel.CloseHandle.argtypes=[ctypes.c_void_p]
        memory.handle=kernel.OpenProcess(0x410,False,os.getpid());self.assertTrue(memory.handle)
        request={'schema_version':1,'base':memory.base,'world_rva':WORLD_RVA,'definition_vtable':0x293C2E0,'joint_focus_vtable':None,'watch':True}
        try:
            sample=NativeSample(memory);packet=read_process_core(sample,request);sample.verify()
            expected=reference(memory)
            for key,value in packet['state'].items():self.assertEqual(value,expected[key],key)
            self.assertGreater(sample.used,0);self.assertTrue(sample.witnesses)
            with self.assertRaisesRegex(LiveReadError,'Limite di lettura'):read_process_core(NativeSample(memory,budget=16),request)
        finally:kernel.CloseHandle(memory.handle)

    def test_focus_research_and_watch_ranges_match_reference(self):
        for completed in (False,True):
            with self.subTest(completed=completed):
                memory=Fixture()
                memory.array(memory.world+0x340,[-1,0],4)
                if completed:
                    memory.put(memory.focus_state+0x10,'<Q',0);memory.array(memory.focus_state+0x40,[memory.focus,memory.focus])
                    memory.put(memory.model+0x174,'<i',1);memory.array(memory.tech_state+0x40,[memory.model]);memory.put(memory.slot+0x18,'<Q',0)
                expected_watch={};expected=reference(memory,watch_plan=expected_watch)
                sample=NativeSample(memory);actual=read_core(sample,watch=True)
                sample.verify()
                for key,value in actual['state'].items():self.assertEqual(value,expected[key],key)
                actual_watch={(address,size):sample.read(address,size) for address,size in actual['ranges']}
                self.assertEqual(actual_watch,expected_watch)

    def test_primary_identity_owners_and_inconsistent_completion_are_rejected(self):
        for fault in ('selector','owner','completion'):
            with self.subTest(fault=fault):
                memory=Fixture()
                if fault=='selector':memory.put(memory.world+0x520,'<i',0)
                elif fault=='owner':memory.put(memory.focus_state+8,'<Q',memory.world)
                else:memory.put(memory.model+0x174,'<i',1)
                with self.assertRaises(LiveReadError):read_core(NativeSample(memory),watch=True)
