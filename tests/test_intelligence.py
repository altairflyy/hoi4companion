"""Synthetic agency layout and invalidation tests; no game writes or calls."""
import unittest

from hoi4companion.country import CountryReader, collect_country
from hoi4companion.contracts import normalise_state
from hoi4companion.live import LiveReadError, compile_watch_reads, watch_changed
from test_native_fixture import Fixture


class IntelligenceFixture(Fixture):
    def __init__(self):
        super().__init__()
        self.agency = self.object('CIntelligenceAgency')
        self.put(self.country + 0xFC0, '<Q', self.agency)
        self.put(self.agency + 0x78, '<Q', self.country)
        self.flags(False, True)

    def flags(self, created, creating):
        self.put(self.agency + 0xC0, '<BB', created, creating)

    def reader(self):
        return CountryReader(self, self.world, self.country, 1)


class IntelligenceTests(unittest.TestCase):
    def test_native_creation_and_operational_flags_are_independent(self):
        fixture = IntelligenceFixture()
        for created, creating in ((False, False), (False, True), (True, False)):
            with self.subTest(created=created, creating=creating):
                fixture.flags(created, creating)
                section = fixture.reader().intelligence()
                self.assertEqual(section['status'], 'observed')
                self.assertIs(section['is_created'], created)
                self.assertIs(section['in_creation'], creating)
                state = normalise_state({'usable': True, 'sample': {
                    'country': 'ROM', 'country_overview': {'country': 'ROM', 'intelligence': section}}})
                self.assertEqual(state['facts']['country.intelligence.in_creation'], {
                    'status': 'observed', 'value': creating, 'source': 'process_memory'})

    def test_invalid_flags_owner_and_type_are_rejected(self):
        for case in ('created', 'creating', 'owner', 'type', 'absent'):
            with self.subTest(case=case):
                fixture = IntelligenceFixture()
                if case == 'created':
                    fixture.flags(2, 0)
                elif case == 'creating':
                    fixture.flags(0, 255)
                elif case == 'owner':
                    fixture.put(fixture.agency + 0x78, '<Q', fixture.world)
                elif case == 'type':
                    fixture.put(fixture.country + 0xFC0, '<Q', fixture.country)
                else:
                    fixture.put(fixture.country + 0xFC0, '<Q', 0)
                with self.assertRaises((LiveReadError, KeyError)):
                    fixture.reader().intelligence()

    def test_failed_agency_is_isolated_and_never_means_not_started(self):
        fixture = IntelligenceFixture()
        fixture.put(fixture.agency + 0x78, '<Q', fixture.world)
        overview = collect_country(fixture, fixture.world, fixture.country, 1, 'ROM')
        self.assertEqual(overview['intelligence']['status'], 'unavailable')
        self.assertNotIn('in_creation', overview['intelligence'])
        state = normalise_state({'usable': True, 'sample': {'country': 'ROM', 'country_overview': overview}})
        self.assertNotIn('country.intelligence.in_creation', state['facts'])

    def test_flag_pointer_and_owner_changes_are_watched_at_unchanged_tick(self):
        for case in ('start', 'complete', 'cancel', 'pointer', 'owner'):
            with self.subTest(case=case):
                fixture = IntelligenceFixture()
                fixture.flags(False, case in ('complete', 'cancel'))
                reader = fixture.reader()
                reader.intelligence()
                reads = compile_watch_reads(reader.watch)
                tick = fixture.read(fixture.world + 0x468, 4)
                self.assertFalse(watch_changed(fixture, reads))
                if case == 'start':
                    fixture.flags(False, True)
                elif case == 'complete':
                    fixture.flags(True, False)
                elif case == 'cancel':
                    fixture.flags(False, False)
                elif case == 'pointer':
                    fixture.put(fixture.country + 0xFC0, '<Q', 0)
                else:
                    fixture.put(fixture.agency + 0x78, '<Q', fixture.world)
                self.assertTrue(watch_changed(fixture, reads))
                self.assertEqual(fixture.read(fixture.world + 0x468, 4), tick)

    def test_reread_rejects_changes_during_sample(self):
        fixture = IntelligenceFixture()
        reader = fixture.reader()
        reader.intelligence()
        fixture.flags(True, False)
        with self.assertRaises(LiveReadError):
            reader.s.verify()


if __name__ == '__main__':
    unittest.main()
