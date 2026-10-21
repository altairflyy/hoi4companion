"""Synthetic structural contracts; these fixtures do not verify game UI values."""
import unittest

from hoi4companion.country import CountryReader, collect_country
from hoi4companion.live import LiveReadError, compile_watch_reads, watch_changed
from test_native_fixture import Fixture


class CountryFixture(Fixture):
    def __init__(self):
        super().__init__()
        self.primary = self.object('CSubUnitDefinition')
        self.string(self.primary + 0x18, 'infantry')
        self.template = self.object('CReferencedDivisionTemplate')
        self.put(self.template + 8, '<II', 10, 20)
        self.string(self.template + 0x20, 'Foreign infantry')
        self.put(self.template + 0x1A8, '<Q', self.primary)
        self.put(self.template + 0x1EC, '<I', 2)  # Legitimate expeditionary template.
        self.units = []
        for number in range(2):
            unit = self.object('CArmy')
            self.put(unit + 0x18, '<II', 51, number + 1)
            self.put(unit + 0x1D8, '<I', 1)
            self.put(unit + 0x3B8, '<Q', self.template)
            self.units.append(unit)
        self.array(self.country + 0x290, self.units)
        self.state = self.object('CState')
        self.put(self.state + 0x58, '<i', 22)
        self.put(self.state + 0xC8, '<i', 1)
        self.put(self.state + 0xCC, '<i', 1)
        self.put(self.state + 0x848, '<i', 700)
        self.array(self.country + 0x460, [self.state])
        self.put(self.country + 0x334, '<i', -100)
        self.put(self.country + 0x348, '<i', 20000)
        self.production_status = self.object('CProductionStatus')
        self.put(self.country + 0xF68, '<Q', self.production_status)
        self.put(self.production_status + 0x290, '<Q', self.country)
        self.line = self.object('CMilitaryProductionLine')
        self.put(self.line + 0x20, '<Q', self.production_status)
        self.variant = self.object('CEquipmentVariant')
        self.string(self.variant + 0x28, 'Rifle variant')
        self.equipment = self.object('CEquipmentType')
        self.string(self.equipment + 0x18, 'infantry_equipment_1')
        self.put(self.variant + 0x3F0, '<Q', self.equipment)
        self.put(self.line + 0x88, '<Q', self.variant)
        self.put(self.line + 0xE8, '<i', 8)
        self.put(self.line + 0x40, '<i', 3)
        self.put(self.line + 0x18, '<i', 5)
        self.put(self.line + 0x28, '<q', 200000)
        self.put(self.line + 0x30, '<q', 750000)
        self.array(self.production_status + 0x58, [self.line])
        for offset, value in ((0x2B8, 5), (0x318, 0), (0x378, 12)):
            self.put(self.production_status + offset, '<q', value * 100000)
        registry = self.allocate()
        self.put(self.base + 0x332EE28, '<Q', registry)
        definitions = []
        for slot, key in enumerate(('infrastructure', 'industrial_complex', 'arms_factory', 'dockyard')):
            definition = self.object('CBuildingTemplate')
            self.string(definition + 0x270, key)
            self.put(definition + 0x2E0, '<i', slot)
            definitions.append(definition)
        self.array(registry + 0x40, definitions)
        self.array(self.world + 0x2C8, [self.state])
        self.array(self.state + 0x140, [-1, 0, 1, -1], 4)
        self.buildings = []
        for level in (3, 4):
            building = self.object('CBuilding')
            self.put(building + 0x40, '<h', level)
            self.buildings.append(building)
        self.array(self.state + 0x158, self.buildings)

    def reader(self):
        return CountryReader(self, self.world, self.country, 1)

    def stockpile(self, duplicate=False, amount=125000):
        pool_type = self.object('CEquipmentVariantPool')
        self.put(self.production_status + 0x200, '<Q', int.from_bytes(self.read(pool_type, 8), 'little'))
        other = self.variant if duplicate else self.object('CEquipmentVariant')
        if not duplicate:
            self.put(other + 0x3F0, '<Q', self.equipment)
            self.string(other + 0x28, 'Second variant')
        data = self.allocate()
        self.put(self.production_status + 0x220, '<Qii', data, 2, 2)
        self.put(data, '<QqQq', self.variant, amount, other, 250000)

    def resources(self):
        self.resource_manager = self.object('CCountryResources')
        self.put(self.country + 0x11F8, '<Q', self.resource_manager)
        self.put(self.resource_manager + 0x20, '<i', 1)
        self.put(self.base + 0x35E1AB0, '<B', 1)
        self.put(self.base + 0x35E1AB4, '<i', 0x30C5)
        self.resource_names = self.allocate()
        self.next_address += 0x100000  # Keep the sparse token table separate.
        self.put(self.base + 0x35E1AE0, '<Q', self.resource_names)
        for token, name in ((1, 'steel'), (2, 'aluminium'), (0x2FAC, 'produced'),
                            (0x30C2, 'imported'), (0x30C3, 'to_use'), (0x30C5, 'exported')):
            self.string(self.resource_names + token * 32, name)
        registry = self.allocate()
        self.put(self.base + 0x332F088, '<Q', registry)
        definitions = [0]
        for token in (1, 2):
            definition = self.object('CStrategicResource')
            self.put(definition + 8, '<i', token)
            self.put(definition + 0xD8, '<i', token)
            definitions.append(definition)
        self.array(registry + 0x28, definitions)
        groups = ((0x28, [(0, 0), (1, 12), (2, 6)]),
                  (0x188, [(2, 1), (0, 0), (1, 0)]),
                  (0x2E8, [(1, -2), (2, 5), (0, 0)]),
                  (0x398, [(2, -1), (1, -3), (0, 0)]),
                  (0x5B0, [(0, 0), (2, 2), (1, 4)]))
        for offset, entries in groups:
            data = self.allocate()
            self.put(self.resource_manager + offset + 8, '<Qii', data, 3, 3)
            for n, (token, amount) in enumerate(entries):
                self.put(data + n * 16, '<qII', amount * 100000, token, 0)


class CountryTest(unittest.TestCase):
    def test_divisions_count_primary_type_and_foreign_expeditionary_template(self):
        fixture = CountryFixture()
        value = fixture.reader().divisions()
        self.assertEqual(value['total'], 2)
        self.assertEqual(value['by_type'], [{'type': 'infantry', 'name': 'infantry', 'count': 2}])
        self.assertEqual(len(value['by_template']), 1)
        self.assertEqual(value['by_template'][0]['name'], 'Foreign infantry')
        self.assertEqual(value['by_template'][0]['count'], 2)

    def test_missing_primary_type_stays_unknown(self):
        fixture = CountryFixture()
        fixture.put(fixture.template + 0x1A8, '<Q', 0)
        self.assertEqual(fixture.reader().divisions()['by_type'][0]['type'], 'unknown')

    def test_available_manpower_adds_country_component_to_states(self):
        value = CountryFixture().reader().manpower()
        self.assertEqual(value['available'], 600)
        self.assertEqual(value['fielded'], 20000)
        self.assertIsNone(value['training'])
        self.assertEqual(value['status'], 'partial')

    def test_negative_manpower_total_is_rejected(self):
        fixture = CountryFixture()
        fixture.put(fixture.country + 0x334, '<i', -1000)
        with self.assertRaises(LiveReadError):
            fixture.reader().manpower()

    def test_production_requested_minus_unassigned_and_cost_output(self):
        value = CountryFixture().reader().production()
        line = value['lines'][0]
        self.assertEqual((line['requested_factories'], line['assigned_factories']), (8, 5))
        self.assertEqual(line['output_per_day'], 3.75)
        self.assertEqual(line['category'], 'military')
        self.assertEqual(value['effective_factories'], {'military': 5, 'dockyards': 0, 'civilian': 12})

    def test_negative_or_impossible_production_values_rejected(self):
        for offset, fmt, value in ((0xE8, '<i', -1), (0x40, '<i', 9),
                                   (0x28, '<q', 0), (0x30, '<q', -1)):
            with self.subTest(offset=offset):
                fixture = CountryFixture()
                fixture.put(fixture.line + offset, fmt, value)
                with self.assertRaises(LiveReadError):
                    fixture.reader().production()

    def test_invalid_array_and_foreign_unit_or_line_owner_rejected(self):
        cases = ('array', 'unit_owner', 'line_owner')
        for case in cases:
            with self.subTest(case=case):
                fixture = CountryFixture()
                if case == 'array':
                    fixture.put(fixture.country + 0x298, '<ii', 1, 2)
                elif case == 'unit_owner':
                    fixture.put(fixture.units[0] + 0x1D8, '<I', 2)
                else:
                    fixture.put(fixture.line + 0x20, '<Q', fixture.country)
                with self.assertRaises(LiveReadError):
                    getattr(fixture.reader(), 'production' if case == 'line_owner' else 'divisions')()

    def test_state_absent_building_is_zero_and_invalid_mapping_is_rejected(self):
        fixture = CountryFixture()
        state = fixture.reader().states()['items'][0]
        self.assertEqual((state['infrastructure'], state['dockyards']), (0, 0))
        self.assertEqual((state['civilian_factories'], state['military_factories']), (3, 4))
        fixture.array(fixture.state + 0x140, [-1, 0, 2, -1], 4)
        with self.assertRaises(LiveReadError):
            fixture.reader().states()

    def test_state_owner_and_controller_are_distinct(self):
        fixture = CountryFixture()
        fixture.put(fixture.state + 0xC8, '<i', 2)
        state = fixture.reader().states()['items'][0]
        self.assertFalse(state['owned'])
        self.assertTrue(state['controlled'])

    def test_stockpile_aggregates_variants_with_fixed_point_scale(self):
        fixture = CountryFixture()
        fixture.stockpile()
        value = fixture.reader().equipment()
        self.assertEqual(value['status'], 'partial')
        self.assertEqual(value['items'][0]['stockpile'], 3.75)
        self.assertEqual([v['stockpile'] for v in value['variants']], [1.25, 2.5])
        self.assertIsNone(value['items'][0]['needed'])
        self.assertIsNone(value['items'][0]['balance_per_day'])

    def test_stockpile_negative_amount_and_duplicate_variant_rejected(self):
        for options in ({'amount': -1}, {'duplicate': True}):
            with self.subTest(options=options):
                fixture = CountryFixture()
                fixture.stockpile(**options)
                with self.assertRaises(LiveReadError):
                    fixture.reader().equipment()

    def test_resource_tokens_join_reordered_fields_and_keep_signed_balances(self):
        fixture = CountryFixture()
        fixture.resources()
        value = fixture.reader().resources()
        resources = {item['id']: item for item in value['items']}
        self.assertEqual(resources['steel']['available'], -2)
        self.assertEqual(resources['steel']['used'], 3)
        self.assertEqual(resources['steel']['balance'], -5)
        self.assertEqual(resources['steel']['produced'], 12)
        self.assertEqual(resources['aluminium']['imported'], 1)
        self.assertEqual(resources['aluminium']['exported'], 2)
        self.assertEqual(resources['aluminium']['balance'], 4)

    def test_resources_reject_bad_field_names_foreign_owner_and_unready_cache(self):
        for problem in ('field_name', 'owner', 'cache'):
            with self.subTest(problem=problem):
                fixture = CountryFixture()
                fixture.resources()
                if problem == 'field_name':
                    fixture.string(fixture.resource_names + 0x30C3 * 32, 'exported')
                elif problem == 'owner':
                    fixture.put(fixture.resource_manager + 0x20, '<i', 2)
                else:
                    fixture.put(fixture.base + 0x35E1AB0, '<B', 0)
                with self.assertRaises(LiveReadError):
                    fixture.reader().resources()

    def test_collect_country_isolates_a_failed_section(self):
        fixture = CountryFixture()
        fixture.put(fixture.line + 0x28, '<q', 0)
        watch = {}
        value = collect_country(fixture, fixture.world, fixture.country, 1, 'ROM', watch)
        self.assertEqual(value['divisions']['status'], 'observed')
        self.assertEqual(value['manpower']['available'], 600)
        self.assertEqual(value['states']['status'], 'observed')
        self.assertEqual(value['production']['status'], 'unavailable')
        self.assertNotIn('lines', value['production'])
        self.assertEqual(value['equipment']['status'], 'unavailable')
        self.assertTrue(watch)

    def test_country_mutations_trigger_watch_while_game_tick_is_unchanged(self):
        for group in ('manpower', 'requested_factories', 'active_factories',
                      'stockpile', 'state_building', 'resources'):
            with self.subTest(group=group):
                fixture = CountryFixture()
                fixture.stockpile()
                fixture.resources()
                watch = {}
                value = collect_country(fixture, fixture.world, fixture.country, 1, 'ROM', watch)
                self.assertTrue(all(value[key]['status'] != 'unavailable'
                                    for key in ('divisions', 'manpower', 'states',
                                                'production', 'equipment', 'resources')))
                reads = compile_watch_reads(watch)
                self.assertFalse(watch_changed(fixture, reads))
                tick = fixture.read(fixture.world + 0x468, 4)
                if group == 'manpower':
                    fixture.put(fixture.country + 0x334, '<i', -99)
                elif group == 'requested_factories':
                    fixture.put(fixture.line + 0xE8, '<i', 9)
                elif group == 'active_factories':
                    fixture.put(fixture.line + 0x18, '<i', 4)
                elif group == 'stockpile':
                    data = int.from_bytes(fixture.read(fixture.production_status + 0x220, 8), 'little')
                    fixture.put(data + 8, '<q', 125001)
                elif group == 'state_building':
                    fixture.put(fixture.buildings[0] + 0x40, '<h', 4)
                else:
                    data = int.from_bytes(fixture.read(fixture.resource_manager + 0x2F0, 8), 'little')
                    fixture.put(data, '<q', -100000)
                self.assertEqual(fixture.read(fixture.world + 0x468, 4), tick)
                self.assertTrue(watch_changed(fixture, reads))


if __name__ == '__main__':
    unittest.main()
