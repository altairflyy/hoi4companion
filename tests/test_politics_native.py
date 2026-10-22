"""Synthetic native membership checks, not proof of game UI presentation."""
import unittest

from hoi4companion.country import CountryReader
from hoi4companion.live import LiveReadError, compile_watch_reads, watch_changed
from hoi4companion.politics_native import read_politics
from test_native_fixture import Fixture


class PoliticsFixture(Fixture):
    def __init__(self):
        super().__init__()
        self.manager = self.object('CPoliticalStatus')
        self.put(self.country + 0xF90, '<Q', self.manager)
        self.group = self.object('CIdeaGroupType')
        self.string(self.group + 0x38, 'trade_laws')
        self.put(self.group + 0x58, '<B', 1)
        self.category = self.object('CIdeaCategory')
        self.string(self.category + 0x10, 'government')
        self.put(self.group + 0x60, '<Q', self.category)
        self.idea = self.object('CIdea')
        self.string(self.idea + 0x18, 'limited_exports')
        self.put(self.idea + 0xAA0, '<Q', self.group)
        self.array(self.manager + 0x50, [self.idea])
        self.characters = self.object('CCountryCharacters')
        self.put(self.country + 0xFF0, '<Q', self.characters)
        self.put(self.characters + 8, '<Q', self.country)
        self.character = self.object('CCharacter')
        self.string(self.character + 0x48, 'TEST_some_person')
        self.put(self.character + 0x68, '<i', 1)
        self.role = self.object('CAdvisor')
        self.put(self.role + 0x10, '<Q', self.character)
        self.put(self.role + 0x28, '<i', 4)
        self.string(self.role + 0x30, 'TEST_some_advisor')
        self.array(self.characters + 0x58, [self.role])

    def reader(self):
        return CountryReader(self, self.world, self.country, 1)


class PoliticsNativeTests(unittest.TestCase):
    def test_exact_membership_and_native_role_identity(self):
        f = PoliticsFixture(); r = f.reader(); result = read_politics(r); r.s.verify()
        self.assertEqual(result['status'], 'observed')
        self.assertEqual(result['ideas']['items'], [dict(id='limited_exports', group='trade_laws', category='government', is_law=True)])
        self.assertEqual(result['advisors']['items'], [dict(id='TEST_some_advisor', character_id='TEST_some_person', slot=4)])

    def test_complete_empty_sets_are_known(self):
        f = PoliticsFixture(); f.array(f.manager + 0x50, []); f.array(f.characters + 0x58, [])
        result = read_politics(f.reader())
        self.assertEqual(result['status'], 'observed')
        self.assertEqual(result['ideas']['items'], [])
        self.assertEqual(result['advisors']['items'], [])

    def test_foreign_character_never_leaks_partial_membership(self):
        f = PoliticsFixture(); f.put(f.character + 0x68, '<i', 2)
        result = read_politics(f.reader())
        self.assertEqual(result['status'], 'partial')
        self.assertEqual(result['ideas']['status'], 'observed')
        self.assertEqual(result['advisors']['status'], 'unavailable')
        self.assertNotIn('items', result['advisors'])

    def test_foreign_manager_rejected(self):
        f = PoliticsFixture(); f.put(f.characters + 8, '<Q', f.world)
        self.assertEqual(read_politics(f.reader())['advisors']['status'], 'unavailable')

    def test_duplicate_idea_or_active_role_rejected(self):
        for kind in ('idea', 'role'):
            with self.subTest(kind=kind):
                f = PoliticsFixture()
                f.array(f.manager + 0x50 if kind == 'idea' else f.characters + 0x58,
                        [f.idea, f.idea] if kind == 'idea' else [f.role, f.role])
                part = 'ideas' if kind == 'idea' else 'advisors'
                self.assertEqual(read_politics(f.reader())[part]['status'], 'unavailable')

    def test_invalid_type_and_law_flag_are_unknown(self):
        for mutation in ('type', 'flag'):
            f = PoliticsFixture()
            if mutation == 'type': f.put(f.idea, '<Q', f.base + 99999)
            else: f.put(f.group + 0x58, '<B', 2)
            self.assertEqual(read_politics(f.reader())['ideas']['status'], 'unavailable')

    def test_two_selected_laws_in_same_group_rejected(self):
        f = PoliticsFixture(); other = f.object('CIdea')
        f.string(other + 0x18, 'free_trade'); f.put(other + 0xAA0, '<Q', f.group)
        f.array(f.manager + 0x50, [f.idea, other])
        self.assertEqual(read_politics(f.reader())['ideas']['status'], 'unavailable')

    def test_same_tick_membership_and_owner_changes_are_watched(self):
        for address_name in ('membership', 'owner', 'role'):
            f = PoliticsFixture(); r = f.reader(); read_politics(r)
            plan = compile_watch_reads(r.watch)
            address = {'membership': f.characters + 0x64, 'owner': f.character + 0x68, 'role': f.role + 0x10}[address_name]
            f.put(address, '<i', 0)
            self.assertTrue(watch_changed(f, plan))

    def test_coherence_recheck_rejects_changed_field(self):
        f = PoliticsFixture(); r = f.reader(); read_politics(r)
        f.put(f.character + 0x68, '<i', 2)
        with self.assertRaises(LiveReadError): r.s.verify()

