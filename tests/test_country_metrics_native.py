"""Synthetic semantic/fault fixtures, not validation against displayed support."""
import hashlib
import struct
import unittest
from unittest.mock import patch

from hoi4companion.country import CountryReader
from hoi4companion.country_metrics_native import (
    COUNTRY_METRICS_LAYOUT, _from_q15, _to_q15, read_metrics,
)
from hoi4companion.live import MILITARY_HASH, MILITARY_LAYOUT, WORLD_RVA, LiveReadError
from test_native_fixture import Fixture


class CountryMetricsNativeTests(unittest.TestCase):
    def setUp(self):
        self.signatures = patch('hoi4companion.country_metrics_native._FUNCTIONS', (
            (0x100, 3, hashlib.sha256(b'abc').hexdigest()),))
        self.signatures.start()
        self.addCleanup(self.signatures.stop)

    def fixture(self):
        f = Fixture()
        f.profile.sha256 = MILITARY_HASH
        f.profile.world_rva = WORLD_RVA
        f.profile.country_metrics_layout = dict(COUNTRY_METRICS_LAYOUT)
        f.profile.military_layout = MILITARY_LAYOUT
        signatures = {**dict(MILITARY_LAYOUT['signatures']), 0x100: b'abc'}
        f.profile.bytes = lambda rva, size: signatures[rva][:size]
        diplomacy = f.object('CDiplomacyStatus')
        f.put(f.country + 0xF88, '<Q', diplomacy)
        f.put(diplomacy + 0x2D0, '<Q', f.country)
        f.array(diplomacy + 0x98, [])
        f.array(diplomacy + 0x20, [])
        threat = f.object('CWorldThreat')
        f.put(f.world + 0x6B0, '<Q', threat)
        f.put(threat + 8, '<q', 25000)
        f.put(f.country + 0x10D8, '<q', 75000)
        for off in (0x14C8, 0x14D0, 0x14D8, 0x14E0):
            f.put(f.country + off, '<q', 0)
        history = f.object('CLoopHistory')
        ring = f.object('CLoopHistoryContainer')
        f.put(f.country + 0xFC8, '<Q', history)
        f.put(history + 0x10, '<Q', ring)
        f.put(ring + 0x38, '<Q', history)
        f.array(ring + 8, [])
        f.put(ring + 0x20, '<iiB', 0, 0, 0)
        f.put(f.country + 0x5B8, '<Qii', 0, 0, 0)
        for rva, value in ((0x33E2D70, 20000), (0x33DF5F0, 32768),
                           (0x33DF678, 10000), (0x33E4528, 100000), (0x33E45D8, 0),
                           (0x33E2C48, 30000), (0x33E2CE0, -20000)):
            f.put(f.base + rva, '<q', value)
        return f, diplomacy, threat, history, ring

    def reader(self, f):
        return CountryReader(f, f.world, f.country, 1)

    def modifier(self, f, amount, flags=0, context=0):
        data = f.allocate()
        f.put(f.country + 0x5B8, '<Qii', data, 1, 1)
        f.put(data, '<IIq', 149, 0, amount)
        definitions = f.allocate()
        f.put(f.base + 0x33DD1D0, '<Q', definitions)
        f.put(definitions + 149 * 0x78 + 0x60, '<I', flags)
        f.put(definitions + 149 * 0x78 + 0x68, '<I', context)
        return data

    def history(self, f, history, ring, values, head, wrapped):
        rows = []
        for value in values:
            row = f.allocate()
            data = f.allocate()
            f.put(row, '<Qii', data, 2, 2)
            f.put(data, '<qq', 0, value)
            f.put(row + 0x18, '<Q', history)
            rows.append(row)
        f.array(ring + 8, rows)
        f.put(ring + 0x20, '<iiB', len(rows), head, wrapped)
        return rows

    def test_effective_support_has_tension_q15_and_modifier_terms(self):
        f, _, _, _, _ = self.fixture()
        self.modifier(f, 10000)
        # A negative Q15 adjustment is rounded with native integer arithmetic.
        f.put(f.country + 0x14C8, '<q', -1638)
        self.assertEqual(read_metrics(self.reader(f)),
                         {'status': 'observed', 'war_support': 0.85001})

    def test_native_fixed_point_negative_rounding_and_conversion(self):
        self.assertEqual(_from_q15(1), 3)
        self.assertEqual(_from_q15(-1), -3)
        self.assertEqual(_from_q15(16384), 50000)
        self.assertEqual(_from_q15(-16384), -50000)
        self.assertEqual(_to_q15(50000), 16384)
        self.assertEqual(_to_q15(-50000), -16384)
        self.assertEqual(_to_q15(-1), 0)

    def test_history_ring_sum_includes_only_live_rows_and_clamps_history_term(self):
        f, _, _, history, ring = self.fixture()
        self.history(f, history, ring, [25000, -5000, 99999], 2, 0)
        self.assertEqual(read_metrics(self.reader(f))['war_support'], 0.9)
        # Wrapped: all rows are included; the head changes ordering only.
        f.put(ring + 0x20, '<iiB', 3, 1, 1)
        f.put(f.base + 0x33DF678, '<q', 100000)
        self.assertEqual(read_metrics(self.reader(f))['war_support'], 1.0)

    def test_support_bounds_zero_and_one_are_observed(self):
        for amount, expected in ((-100000, 0.0), (200000, 1.0)):
            f, _, _, _, _ = self.fixture()
            f.put(f.country + 0x10D8, '<q', amount)
            self.assertEqual(read_metrics(self.reader(f))['war_support'], expected)

    def test_modifier_definition_bounds_apply_and_context_cannot_be_guessed(self):
        f, _, _, _, _ = self.fixture()
        self.modifier(f, -10000, flags=0x10)
        self.assertEqual(read_metrics(self.reader(f))['war_support'], 0.8)
        f, _, _, _, _ = self.fixture()
        self.modifier(f, 10000, context=1)
        with self.assertRaisesRegex(LiveReadError, 'context'):
            read_metrics(self.reader(f))

    def war_relation(self, f, diplomacy, left=1, right=2, swapped=0, exclusion=0):
        opponent = f.object('CCountry')
        f.put(opponent + 8, '<i', 2)
        other_diplomacy = f.object('CDiplomacyStatus')
        f.put(opponent + 0xF88, '<Q', other_diplomacy)
        f.put(other_diplomacy + 0x2D0, '<Q', opponent)
        f.put(other_diplomacy + 0x1F0, '<i', exclusion)
        f.array(f.world + 0x310, [f.country, opponent])
        f.array(f.world + 0x340, [0, 0, 1, 0], 4)
        relation = f.object('CWarRelation')
        f.put(relation + 8, '<i', 0x380A)
        f.put(relation + 0x10, '<ii', left, right)
        f.put(relation + 0x158, '<B', swapped)
        f.array(diplomacy + 0x20, [relation])
        return relation

    def test_war_sides_shared_relations_and_country_tag_aliases(self):
        for left, right, swapped, expected in (
                (1, 2, 0, 0.6), (1, 2, 1, 1.0),
                (2, 1, 1, 0.6), (2, 1, 0, 1.0), (3, 2, 0, 0.6)):
            with self.subTest(left=left, right=right, swapped=swapped):
                f, diplomacy, _, _, _ = self.fixture()
                self.war_relation(f, diplomacy, left, right, swapped)
                self.assertEqual(read_metrics(self.reader(f))['war_support'], expected)

    def test_getter_war_predicate_uses_fixed_right_country_and_not_cached_count(self):
        f, diplomacy, _, _, _ = self.fixture()
        self.war_relation(f, diplomacy, exclusion=1)
        f.array(diplomacy + 0x98, [f.object('CWar')])
        self.assertEqual(read_metrics(self.reader(f))['war_support'], 0.8)
        f, diplomacy, _, _, _ = self.fixture()
        relation = self.war_relation(f, diplomacy)
        second = f.object('CWarRelation')
        f.put(second + 8, '<i', 0x380A)
        f.put(second + 0x10, '<ii', 1, 2)
        f.put(second + 0x158, '<B', 1)
        f.array(diplomacy + 0x20, [relation, second])
        self.assertEqual(read_metrics(self.reader(f))['war_support'], 0.9)
        r = self.reader(f)
        read_metrics(r)
        self.assertIn((relation + 0x158, 1), r.watch)
        f.put(relation + 0x158, '<B', 2)
        with self.assertRaises(LiveReadError):
            read_metrics(self.reader(f))

    def test_invalid_country_profile_and_bounds_fail_closed(self):
        mutations = (
            lambda f, d, t, h, r: f.put(f.country + 8, '<i', 2),
            lambda f, d, t, h, r: f.put(f.world + 0x520, '<i', 2),
            lambda f, d, t, h, r: setattr(f.profile, 'sha256', 'other'),
            lambda f, d, t, h, r: setattr(f.profile, 'country_metrics_layout', None),
            lambda f, d, t, h, r: setattr(f.profile, 'bytes', lambda rva, size: b'\0' * size),
            lambda f, d, t, h, r: f.put(f.base + 0x33E4528, '<q', 100),
            lambda f, d, t, h, r: f.put(t + 8, '<q', -1),
            lambda f, d, t, h, r: f.put(r + 0x38, '<Q', h + 0x2000),
            lambda f, d, t, h, r: f.put(r + 0x28, '<B', 2),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                args = self.fixture()
                mutation(*args)
                with self.assertRaises(LiveReadError):
                    read_metrics(self.reader(args[0]))

    def test_selected_country_mapping_and_history_row_owner_are_required(self):
        f, _, _, history, ring = self.fixture()
        rows = self.history(f, history, ring, [1000], 0, 1)
        f.put(rows[0] + 0x18, '<Q', history + 0x2000)
        with self.assertRaises(LiveReadError):
            read_metrics(self.reader(f))
        f, _, _, _, _ = self.fixture()
        f.array(f.world + 0x340, [0, -1], 4)
        with self.assertRaises(LiveReadError):
            read_metrics(self.reader(f))

    def test_every_dependency_joins_watch_and_incoherent_sample_is_rejected(self):
        f, _, threat, history, ring = self.fixture()
        rows = self.history(f, history, ring, [1000], 0, 1)
        modifier = self.modifier(f, 10000)
        r = self.reader(f)
        read_metrics(r)
        r.s.verify()
        for key in ((f.country + 0x10D8, 8), (threat + 8, 8),
                    (f.country + 0x14C8, 8), (ring + 0x20, 8),
                    (ring + 0x28, 1), (modifier, 16), (rows[0], 16),
                    (f.base + 0x33E2D70, 8), (f.world + 0x520, 4)):
            self.assertIn(key, r.watch)
        f.put(threat + 8, '<q', 25001)
        with self.assertRaises(LiveReadError):
            r.s.verify()


if __name__ == '__main__':
    unittest.main()
