from copy import deepcopy
from unittest import TestCase

from hoi4companion.backend_core import _request
from hoi4companion._operation_import_reference import bind_operations
from hoi4companion.resolution import catalog_index
from test_operation_import import CATALOG


class NativeOperationBindingTests(TestCase):
    def test_whole_pass_matches_existing_components_and_is_idempotent(self):
        entries = CATALOG + [
            {'kind': 'project', 'id': 'vehicles', 'name': 'Engineering Vehicles'},
            {'kind': 'equipment', 'id': 'rifles', 'name': 'Infantry Equipment', 'archetype': 'infantry_equipment'},
            {'kind': 'law', 'id': 'civilian', 'name': 'Civilian Economy', 'group': 'economic_laws'},
        ]
        params = [
            {'operation_id': 'create_intelligence_agency'},
            {'category': 'projects', 'project_name': 'Engineering Vehicles'},
            {'category': 'construction', 'building_id': 'arms_factory', 'state_id': 77,
             'quantity': 2, 'quantity_mode': 'additional'},
            {'category': 'production', 'equipment_name': 'Infantry Equipment', 'factories': 5},
            {'category': 'politics', 'law_name': 'Civilian Economy'},
            {'category': 'checkpoints', 'instructions': ['Do this when ready']},
        ]
        guide = {'tracks': {'operations': [
            {'key': f'order-{i}', 'action': {'kind': 'manual', 'params': p}}
            for i, p in enumerate(params)
        ]}}
        guide['tracks']['operations'].append({'key': 'reminder', 'tracking': 'reminder',
            'action': {'kind': 'manual', 'params': {}}})
        index = catalog_index(entries)
        expected = deepcopy(guide)
        reports = bind_operations(expected, index)
        native = _request(0, 'import_bind_operations', guide=guide,
                          library=index.reference_designs, **index.native_import_args())
        self.assertEqual(native, {'guide': expected, 'reports': reports})
        rebound = deepcopy(expected)
        rebound_reports = bind_operations(rebound, index)
        self.assertEqual(rebound, expected)
        self.assertEqual(_request(0, 'import_bind_operations', guide=expected,
                         library=index.reference_designs, **index.native_import_args()),
                         {'guide': rebound, 'reports': rebound_reports})
