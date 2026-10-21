"""Structural fault tests; synthetic addresses are not a game-state oracle."""
import struct
import unittest
from types import SimpleNamespace
from hoi4companion.live import read_native_state, LiveReadError, WORLD_RVA, ExecutableProfile


class Fixture:
    base = 0x140000000
    pid = 123
    created = 456

    def __init__(self):
        self.memory = {}
        self.next_address = 0x200000000
        self.classes = {}
        self.profile = SimpleNamespace(sha256='fixture', class_name=lambda rva: self.classes[rva])
        self.world = self.object('CCurrentGameState')
        self.put(self.base + WORLD_RVA, '<Q', self.world)
        self.put(self.world + 0x468, '<i', 60806040)
        self.put(self.world + 0x520, '<i', 1)
        tags = self.allocate()
        self.put(self.world + 0x358, '<Q', tags)
        self.put(self.world + 0x364, '<i', 2)
        self.string(tags + 32, 'ROM')
        self.array(self.world + 0x340, [0, 0], 4)
        self.country = self.object('CCountry')
        self.put(self.country + 8, '<i', 1)
        self.array(self.world + 0x310, [self.country])
        self.tree = self.object('CNationalFocusTree')
        self.string(self.tree + 8, 'romanian_focus')
        self.put(self.country + 0x1370, '<Q', self.tree)
        self.focus = self.object('CNationalFocus')
        self.string(self.focus + 0x18, 'ROM_test')
        self.put(self.focus + 0x160, '<Q', self.tree)
        self.array(self.tree + 0x30, [self.focus])
        self.focus_state = self.object('CNationalFocusProgress')
        self.put(self.country + 0x1380, '<Q', self.focus_state)
        self.put(self.focus_state + 8, '<Q', self.country)
        self.put(self.focus_state + 0x10, '<Q', self.focus)
        self.continuous = self.object('CContinuousNationalFocus')
        self.string(self.continuous + 0x18, 'continuous_test')
        self.tech_state = self.object('CTechnologyStatus')
        self.put(self.country + 0xF60, '<Q', self.tech_state)
        self.model = self.object('CTechnology')
        self.put(self.model + 0x158, '<Q', self.tech_state)
        self.definition = self.allocate()
        self.put(self.definition, '<Q', self.base + 0x293C2E0)
        self.put(self.definition + 0x38, '<i', 1)
        self.put(self.definition + 0x3DC, '<i', 1)
        self.string(self.definition + 0x10, 'construction4')
        self.put(self.model + 0x160, '<Q', self.definition)
        self.array(self.tech_state + 0x88, [self.model])
        self.slot = self.object('CResearchSlot')
        self.put(self.slot + 0x10, '<Q', self.tech_state)
        self.put(self.slot + 0x18, '<Q', self.model)
        self.array(self.tech_state + 0xA0, [self.slot])

    def allocate(self):
        address = self.next_address
        self.next_address += 0x2000
        return address

    def put(self, address, fmt, *values):
        for offset, byte in enumerate(struct.pack(fmt, *values)):
            self.memory[address + offset] = byte

    def object(self, name):
        address = self.allocate()
        rva = len(self.classes) * 32 + 0x1000
        self.classes[rva] = f'.?AV{name}@@'
        self.put(address, '<Q', self.base + rva)
        return address

    def array(self, address, values, width=8):
        data = self.allocate()
        self.put(address, '<Qii', data, len(values), len(values))
        for index, value in enumerate(values):
            self.put(data + index * width, '<Q' if width == 8 else '<i', value)

    def string(self, address, value):
        raw = value.encode() + b'\0'
        pointer = address if len(raw) <= 16 else self.allocate()
        self.put(address + 16, '<QQ', len(raw) - 1, 15 if pointer == address else 255)
        if pointer != address:
            self.put(address, '<Q', pointer)
        for index, byte in enumerate(raw):
            self.memory[pointer + index] = byte

    def read(self, address, size):
        return bytes(self.memory.get(address + i, 0) for i in range(size))

    def identity(self):
        return self.created


class NativeFixtureTest(unittest.TestCase):
    def test_current_focus_and_research_are_owned_by_selected_country(self):
        value = read_native_state(Fixture())
        self.assertEqual(value['country'], 'ROM')
        self.assertEqual(value['current_focus'], 'ROM_test')
        self.assertEqual(value['research_slots'][0]['technology_id'], 'construction4')
        self.assertEqual(value['researched_technologies'], [])

    def test_continuous_pointer_is_not_interpreted_as_progress(self):
        mem = Fixture()
        mem.put(mem.focus_state + 0x10, '<Q', 0)
        mem.put(mem.focus_state + 0x18, '<Q', mem.continuous)
        value = read_native_state(mem)
        self.assertEqual(value['continuous_focus'], 'continuous_test')
        self.assertIsNone(value['focus_progress'])

    def test_rejects_foreign_owners_for_focus_and_research(self):
        for owner in ('focus', 'slot', 'model'):
            with self.subTest(owner=owner):
                mem = Fixture()
                address = {'focus': mem.focus_state + 8, 'slot': mem.slot + 0x10,
                           'model': mem.model + 0x158}[owner]
                mem.put(address, '<Q', mem.world)
                with self.assertRaises(LiveReadError):
                    read_native_state(mem)

    def test_completed_focus_must_belong_to_tree(self):
        mem = Fixture()
        mem.array(mem.focus_state + 0x40, [mem.model])
        with self.assertRaises(LiveReadError):
            read_native_state(mem)

    def test_research_completion_list_and_level_must_agree(self):
        mem = Fixture()
        mem.put(mem.model + 0x174, '<i', 1)
        with self.assertRaises(LiveReadError):
            read_native_state(mem)
        mem.array(mem.tech_state + 0x40, [mem.model])
        mem.put(mem.slot + 0x18, '<Q', 0)
        self.assertEqual(read_native_state(mem)['researched_technologies'], ['construction4'])

    def test_active_completed_focus_is_rejected(self):
        mem = Fixture()
        mem.array(mem.focus_state + 0x40, [mem.focus])
        with self.assertRaises(LiveReadError):
            read_native_state(mem)

    def test_changed_primary_country_selector_invalidates_entire_sample(self):
        mem = Fixture()
        original = mem.read
        calls = 0
        def read(address, size):
            nonlocal calls
            if address == mem.world + 0x520:
                calls += 1
                if calls > 1:
                    return struct.pack('<i', 2)
            return original(address, size)
        mem.read = read
        with self.assertRaises(LiveReadError):
            read_native_state(mem)

    def test_unknown_executable_is_rejected(self):
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'hoi4.exe'
            path.write_bytes(b'unsupported')
            with self.assertRaisesRegex(LiveReadError, 'Versione HOI4'):
                ExecutableProfile(path)
