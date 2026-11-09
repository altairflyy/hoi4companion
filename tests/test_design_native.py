"""Structural fault tests; fixtures do not substitute for live evidence."""
import struct
import unittest

from hoi4companion.country import CountryReader
from hoi4companion.design_native import (read_design, SIGNATURES, TOKEN_INITIALIZED,
                                        TOKEN_MAXIMUM, TOKEN_TABLE, MAX_ENTRIES)
from hoi4companion.live import MILITARY_HASH, LiveReadError
from test_native_fixture import Fixture


class DesignNativeTests(unittest.TestCase):
    def fixture(self):
        f = Fixture()
        f.profile.sha256 = MILITARY_HASH
        f.profile.bytes = lambda rva, size: dict(SIGNATURES)[rva][:size]
        table = f.allocate()
        f.put(f.base + TOKEN_INITIALIZED, '<B', 1)
        f.put(f.base + TOKEN_MAXIMUM, '<i', 60000)
        f.put(f.base + TOKEN_TABLE, '<Q', table)
        for token, value in ((0x3069, 'upgrades'), (0x3B6B, 'modules'),
                             (50000, 'main_armament_slot'), (50001, 'tank_medium_howitzer'),
                             (50002, 'tank_nsb_armor_upgrade'), (50003, 'special_type_slot_1')):
            f.string(table + token * 32, value)
        variant = f.object('CEquipmentVariant')
        equipment = f.object('CEquipmentType')
        f.put(variant + 0x3F0, '<Q', equipment)
        instance = f.object('CEquipmentUpgradesInstance')
        f.put(variant + 0x68, '<Q', struct.unpack('<Q', f.read(instance, 8))[0])
        module = f.object('CEquipmentModule')
        f.put(module + 8, '<i', 50001)
        f.put(module + 0x10, '<B', 1)
        upgrade = f.object('CEquipmentUpgrade')
        f.put(upgrade + 8, '<i', 50002)
        modules, upgrades = f.allocate(), f.allocate()
        f.put(variant + 0xB8, '<Qii', modules, 2, 2)
        f.put(modules, '<I4xQ', 50000, module)
        f.put(modules + 16, '<I4xQ', 50003, 0)
        f.put(variant + 0x70, '<Qii', upgrades, 1, 1)
        f.put(upgrades, '<QB7x', upgrade, 4)
        return f, variant, module, upgrade, modules, upgrades

    def reader(self, f):
        return CountryReader(f, f.world, f.country, 1)

    def test_complete_slot_mapping_and_native_integer_upgrade(self):
        f, variant, _, _, modules, upgrades = self.fixture()
        r = self.reader(f)
        self.assertEqual(read_design(r, variant), {
            'status': 'observed', 'modules': {'main_armament_slot': 'tank_medium_howitzer'},
            'upgrades': {'tank_nsb_armor_upgrade': 4}})
        for address, size in ((variant + 0x70, 16), (variant + 0xB8, 16),
                              (modules, 32), (upgrades, 16)):
            self.assertIn((address, size), r.watch)
        r.s.verify()
        f.put(upgrades + 8, '<B', 5)
        with self.assertRaises(LiveReadError):
            r.s.verify()

    def test_empty_complete_containers_and_zero_levels_are_observations(self):
        f, variant, _, _, _, upgrades = self.fixture()
        f.put(upgrades + 8, '<B', 0)
        self.assertEqual(read_design(self.reader(f), variant)['upgrades'], {'tank_nsb_armor_upgrade': 0})
        for offset in (0x70, 0xB8):
            f.put(variant + offset, '<Qii', 0, 0, 0)
        self.assertEqual(read_design(self.reader(f), variant), {'status': 'observed', 'modules': {}, 'upgrades': {}})

    def test_faults_never_publish_a_prefix_or_inferred_empty_design(self):
        mutations = (
            lambda f,v,m,u,ms,us:setattr(f.profile, 'sha256', 'unsupported'),
            lambda f,v,m,u,ms,us:setattr(f.profile, 'bytes', lambda rva,size:b'\0'*size),
            lambda f,v,m,u,ms,us:f.put(v + 0xB8, '<Qii', ms, 1, 2),
            lambda f,v,m,u,ms,us:f.put(v + 0xB8, '<Qii', 0, 1, 1),
            lambda f,v,m,u,ms,us:f.put(v + 0x70, '<Qii', us, MAX_ENTRIES + 1, 1),
            lambda f,v,m,u,ms,us:f.put(v + 0x70, '<Qii', us, 1, -1),
            lambda f,v,m,u,ms,us:f.put(ms + 16, '<I4xQ', 50000, m),
            lambda f,v,m,u,ms,us:f.put(v + 0x70, '<Qii', us, 2, 2) or f.put(us + 16, '<QB7x', u, 2),
            lambda f,v,m,u,ms,us:f.put(m + 0x10, '<B', 0),
            lambda f,v,m,u,ms,us:f.put(m, '<Q', struct.unpack('<Q', f.read(u,8))[0]),
            lambda f,v,m,u,ms,us:f.put(f.base + TOKEN_INITIALIZED, '<B', 0),
            lambda f,v,m,u,ms,us:f.put(f.base + TOKEN_MAXIMUM, '<i', 50000),
            lambda f,v,m,u,ms,us:f.put(u + 8, '<i', -1),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                args = self.fixture()
                mutation(*args)
                result = read_design(self.reader(args[0]), args[1])
                self.assertEqual(result['status'], 'unavailable')
                self.assertNotIn('modules', result)
                self.assertNotIn('upgrades', result)

    def test_late_read_failure_and_label_change_are_witnessed(self):
        f, variant, _, _, _, upgrades = self.fixture()
        original = f.read
        def read(address, size):
            if address == upgrades:
                raise LiveReadError('late upgrade read failure')
            return original(address, size)
        f.read = read
        self.assertEqual(read_design(self.reader(f), variant)['status'], 'unavailable')
        f, variant, *_ = self.fixture()
        r = self.reader(f)
        self.assertEqual(read_design(r, variant)['status'], 'observed')
        f.put(f.base + TOKEN_TABLE, '<Q', 0)
        with self.assertRaises(LiveReadError):
            r.s.verify()
