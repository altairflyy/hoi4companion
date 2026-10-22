"""Synthetic queue contracts; these tests do not qualify a real game session."""
import unittest

from hoi4companion.contracts import normalise_state
from hoi4companion.country import CountryReader, collect_country
from hoi4companion.live import LiveReadError, compile_watch_reads, watch_changed
from test_native_fixture import Fixture


class ConstructionFixture(Fixture):
    def __init__(self, provincial=False):
        super().__init__()
        self.production = self.object('CProductionStatus')
        self.put(self.country + 0xF68, '<Q', self.production)
        self.put(self.production + 0x290, '<Q', self.country)
        self.registry = self.object('CBuildingDatabase')
        self.put(self.base + 0x332EE28, '<Q', self.registry)
        self.definition = self.object('CBuildingTemplate')
        self.string(self.definition + 0x270, 'arms_factory')
        self.put(self.definition + 0x2E0, '<i', 0)
        self.array(self.registry + 0x40, [self.definition])
        self.state = self.object('CState')
        self.put(self.state + 0x58, '<i', 1)
        self.put(self.state + 0xC8, '<ii', 1, 1)
        self.array(self.world + 0x2C8, [0, self.state])
        self.province = None
        if provincial:
            self.province = self.object('CProvince')
            self.put(self.province + 0xA4, '<i', 2)
            self.put(self.province + 0xC0, '<Q', self.state)
            self.put(self.province + 0x188, '<i', 1)
            self.array(self.world + 0x2B0, [0, 0, self.province])
            self.status = self.province + 0x190
        else:
            self.status = self.state + 0x120
        status_type = self.object('CBuildingStatus')
        self.put(self.status, '<Q', int.from_bytes(self.read(status_type, 8), 'little'))
        self.put(self.status + 0x68, '<iii', int(provincial), 2 if provincial else 1, 0)
        self.building = self.object('CBuilding')
        self.put(self.building + 0x1D8, '<QQ', self.status, self.definition)
        self.put(self.building + 0x40, '<h', 3)
        self.array(self.status + 0x20, [0], 4)
        self.array(self.status + 0x38, [self.building])
        self.line = self.new_line(1)
        self.array(self.production + 0x70, [self.line])

    def new_line(self, identity):
        line = self.object('CBuildingProductionLine')
        self.put(line + 8, '<II', 0x3B, identity)
        self.put(line + 0x18, '<i', 0)
        self.put(line + 0x20, '<Q', self.production)
        self.put(line + 0x48, '<i', 2)
        self.put(line + 0x70, '<Q', self.building)
        return line

    def reader(self):
        return CountryReader(self, self.world, self.country, 1)


def membership(value):
    wire = {'usable': True, 'sample': {'country': 'ROM', 'country_overview': {'country': 'ROM', 'construction': value}}}
    return normalise_state(wire)['facts']['construction.queued']


class ConstructionTest(unittest.TestCase):
    def test_observed_empty_queue_needs_owner_but_no_building_registry(self):
        fixture = ConstructionFixture()
        fixture.array(fixture.production + 0x70, [])
        fixture.put(fixture.base + 0x332EE28, '<Q', 0)
        value = fixture.reader().construction()
        self.assertEqual((value['status'], value['lines']), ('observed', []))
        self.assertEqual(membership(value)['value'], [])
        fixture.put(fixture.production + 0x290, '<Q', fixture.country + 0x100)
        with self.assertRaisesRegex(LiveReadError, 'altra nazione'):
            fixture.reader().construction()

    def test_queued_order_without_assigned_factories_is_observed(self):
        fixture = ConstructionFixture()
        reader = fixture.reader()
        value = reader.construction()
        reader.s.verify()
        self.assertEqual(value['status'], 'observed')
        row = value['lines'][0]
        self.assertEqual((row['building_id'], row['state_id'], row['province_id']), ('arms_factory', 1, None))
        self.assertEqual((row['amount'], row['active_factories'], row['built_level']), (2, 0, 3))
        self.assertEqual((row['repair_levels'], row['conversion'], row['location_status']), (0, False, 'observed'))
        self.assertEqual(membership(value)['value'], ['building:arms_factory', 'state:1:arms_factory'])

    def test_provincial_building_requires_reciprocal_state_and_province(self):
        fixture = ConstructionFixture(provincial=True)
        row = fixture.reader().construction()['lines'][0]
        self.assertEqual((row['state_id'], row['province_id'], row['location_status']), (1, 2, 'observed'))
        fixture.put(fixture.province + 0xA4, '<i', 3)
        with self.assertRaisesRegex(LiveReadError, 'provincia'):
            fixture.reader().construction()

    def test_foreign_line_rejects_section_without_partial_rows(self):
        fixture = ConstructionFixture()
        other = fixture.new_line(2)
        fixture.put(other + 0x20, '<Q', fixture.production + 0x100)
        fixture.array(fixture.production + 0x70, [fixture.line, other])
        with self.assertRaisesRegex(LiveReadError, 'altra nazione'):
            fixture.reader().construction()
        value = collect_country(fixture, fixture.world, fixture.country, 1, 'ROM')['construction']
        self.assertEqual(value['status'], 'unavailable')
        self.assertNotIn('lines', value)

    def test_unsupported_queue_type_is_partial_never_observed_empty(self):
        for mixed in (False, True):
            with self.subTest(mixed=mixed):
                fixture = ConstructionFixture()
                railway = fixture.object('CRailwayProductionLine')
                fixture.array(fixture.production + 0x70, [fixture.line, railway] if mixed else [railway])
                value = fixture.reader().construction()
                self.assertEqual(value['status'], 'partial')
                self.assertEqual(len(value['lines']), int(mixed))
                self.assertEqual(membership(value)['status'], 'unknown')

    def test_invalid_order_fields_are_rejected(self):
        faults = (
            ('handle type', 'line', 8, '<II', (0x3A, 1)),
            ('handle sequence', 'line', 8, '<II', (0x3B, 0)),
            ('negative amount', 'line', 0x48, '<i', (-1,)),
            ('excessive amount', 'line', 0x48, '<i', (1001,)),
            ('negative repairs', 'line', 0x78, '<i', (-1,)),
            ('excessive repairs', 'line', 0x78, '<i', (1001,)),
            ('conversion flag', 'line', 0x8C, '<B', (2,)),
            ('negative active factories', 'line', 0x18, '<i', (-1,)),
            ('negative level', 'building', 0x40, '<h', (-1,)),
            ('excessive level', 'building', 0x40, '<h', (1001,)),
            ('template ordinal', 'definition', 0x2E0, '<i', (1,)),
            ('location identifier', 'status', 0x6C, '<i', (0,)),
            ('state identity', 'state', 0x58, '<i', (2,)),
        )
        for label, attr, offset, fmt, values in faults:
            with self.subTest(fault=label):
                fixture = ConstructionFixture()
                fixture.put(getattr(fixture, attr) + offset, fmt, *values)
                with self.assertRaises(LiveReadError):
                    fixture.reader().construction()

    def test_duplicate_handles_and_invalid_template_membership_are_rejected(self):
        fixture = ConstructionFixture()
        fixture.array(fixture.production + 0x70, [fixture.line, fixture.new_line(1)])
        with self.assertRaisesRegex(LiveReadError, 'duplicata'):
            fixture.reader().construction()
        fixture = ConstructionFixture()
        other = fixture.object('CBuildingTemplate')
        fixture.put(other + 0x2E0, '<i', 0)
        fixture.put(fixture.building + 0x1E0, '<Q', other)
        with self.assertRaisesRegex(LiveReadError, 'catalogo'):
            fixture.reader().construction()

    def test_invalid_building_map_and_reciprocal_location_are_rejected(self):
        for variant in ('absent', 'out_of_range', 'different_building', 'different_state'):
            with self.subTest(variant=variant):
                fixture = ConstructionFixture()
                if variant in ('absent', 'out_of_range'):
                    fixture.array(fixture.status + 0x20, [-1 if variant == 'absent' else 1], 4)
                elif variant == 'different_building':
                    fixture.array(fixture.status + 0x38, [fixture.object('CBuilding')])
                else:
                    fixture.array(fixture.world + 0x2C8, [0, fixture.object('CState')])
                with self.assertRaises(LiveReadError):
                    fixture.reader().construction()

    def test_foreign_territory_and_country_override_remain_partial(self):
        for variant in ('owner', 'controller', 'override', 'province_controller'):
            with self.subTest(variant=variant):
                fixture = ConstructionFixture(provincial=variant == 'province_controller')
                address = {'owner': fixture.state + 0xC8, 'controller': fixture.state + 0xCC,
                           'override': fixture.status + 0x70,
                           'province_controller': (fixture.province or 0) + 0x188}[variant]
                fixture.put(address, '<i', 2)
                value = fixture.reader().construction()
                self.assertEqual(value['status'], 'partial')
                self.assertEqual(len(value['lines']), 1)
                self.assertIsNone(value['lines'][0]['state_id'])
                self.assertEqual(value['lines'][0]['location_status'], 'unavailable')
                self.assertEqual(membership(value)['status'], 'unknown')

    def test_repair_and_conversion_membership_do_not_mean_ordinary_orders(self):
        for attr, offset, fmt in (('repair_levels', 0x78, '<i'), ('conversion', 0x8C, '<B')):
            with self.subTest(kind=attr):
                fixture = ConstructionFixture()
                fixture.put(fixture.line + offset, fmt, 1)
                value = fixture.reader().construction()
                self.assertEqual(value['status'], 'observed')
                self.assertEqual(value['lines'][0][attr], 1)
                self.assertEqual(membership(value)['value'], [])

    def test_witness_reread_rejects_changes_after_decoding(self):
        fixture = ConstructionFixture()
        reader = fixture.reader()
        reader.construction()
        fixture.put(fixture.line + 0x48, '<i', 3)
        with self.assertRaisesRegex(LiveReadError, 'incoerente'):
            reader.s.verify()

    def test_queue_and_level_changes_are_watched_without_tick_advance(self):
        for mutation in ('amount', 'membership', 'enqueue_from_empty', 'built_level'):
            with self.subTest(mutation=mutation):
                fixture = ConstructionFixture()
                if mutation == 'enqueue_from_empty':
                    fixture.array(fixture.production + 0x70, [])
                reader = fixture.reader()
                reader.construction()
                reads = compile_watch_reads(reader.watch)
                self.assertFalse(watch_changed(fixture, reads))
                tick = fixture.read(fixture.world + 0x468, 4)
                if mutation == 'amount':
                    fixture.put(fixture.line + 0x48, '<i', 3)
                elif mutation == 'membership':
                    fixture.array(fixture.production + 0x70, [])
                elif mutation == 'enqueue_from_empty':
                    fixture.array(fixture.production + 0x70, [fixture.line])
                else:
                    fixture.put(fixture.building + 0x40, '<h', 4)
                self.assertEqual(fixture.read(fixture.world + 0x468, 4), tick)
                self.assertTrue(watch_changed(fixture, reads))


if __name__ == '__main__':
    unittest.main()
