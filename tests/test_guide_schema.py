import unittest

from hoi4companion.guide import normalise_guide


def sample():
    return {
        'id': 'example', 'schema_version': 1, 'applies_to': {},
        'sources': [{'id': 'video', 'type': 'youtube', 'url': 'https://example.com/watch'}],
        'phases': [{'id': 'startup', 'title': 'Preparazione'}],
        'tracks': {
            'focus': [{'key': 'first', 'kind': 'gate', 'phase': 'startup',
                       'evidence': [{'source_id': 'video', 'basis': 'explicit', 'timestamp_seconds': 13}]}],
            'research': [{'key': 'second', 'kind': 'gate', 'depends_on': ['first'],
                          'completion': {'op': 'check', 'key': 'done_second'}}],
        },
    }


class GuideSchemaExtensionTests(unittest.TestCase):
    def test_additive_fields_and_cross_track_dependencies(self):
        data = normalise_guide(sample())
        self.assertEqual(data['schema_version'], 1)
        self.assertEqual(data['tracks']['research'][0]['depends_on'], ['first'])
        self.assertEqual(data['tracks']['focus'][0]['evidence'][0]['timestamp_seconds'], 13)

    def test_unknown_dependency_and_phase_fail(self):
        for field, value in [('depends_on', ['missing']), ('phase', 'other')]:
            with self.subTest(field=field):
                data = sample()
                data['tracks']['research'][0][field] = value
                with self.assertRaises(ValueError):
                    normalise_guide(data)

    def test_cycle_across_tracks_fails(self):
        data = sample()
        data['tracks']['focus'][0]['depends_on'] = ['second']
        with self.assertRaisesRegex(ValueError, 'cicliche'):
            normalise_guide(data)

    def test_evidence_references_and_time_ranges_fail_closed(self):
        for changes in [
            {'source_id': 'missing'},
            {'basis': 'invented'},
            {'timestamp_seconds': -1},
            {'end_seconds': 2},
        ]:
            with self.subTest(changes=changes):
                data = sample()
                data['tracks']['focus'][0]['evidence'][0].update(changes)
                with self.assertRaises(ValueError):
                    normalise_guide(data)

    def test_parameters_are_json_safe(self):
        good = sample()
        good['tracks']['focus'][0]['action'] = {'kind': 'manual', 'params': {'quantity': 3}}
        self.assertEqual(normalise_guide(good)['tracks']['focus'][0]['action']['params'], {'quantity': 3})
        bad = sample()
        bad['tracks']['focus'][0]['action'] = {'kind': 'manual', 'params': {'bad': float('nan')}}
        with self.assertRaises(ValueError):
            normalise_guide(bad)

    def test_existing_legacy_guide_stays_valid(self):
        from hoi4companion.guide import load_guide
        old = load_guide()
        self.assertEqual(len(old['tracks']['focus']), 34)
        self.assertEqual(len(old['tracks']['research']), 48)


if __name__ == '__main__':
    unittest.main()
