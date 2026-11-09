"""Presentation packets stay independent of the browser and native reader."""
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from hoi4companion.backend_core import _request
from hoi4companion.state import read_snapshot

class BackendCatalogViewsTests(unittest.TestCase):
    def test_snapshot_json_text_and_unknown_country_preserve_qualified_state(self):
        with TemporaryDirectory() as folder:
            path=Path(folder)/'state.json';path.write_text(json.dumps({'tag':'HUN','completed_focuses':['a','a'],'current_focus':'b','focus_progress':0.5}))
            self.assertEqual(_request(0,'source_snapshot',path=str(path),country='AUS'),read_snapshot(path,'AUS').to_json())
            path=path.with_suffix('.hoi4');path.write_text('HOI4txt countries={HUN={completed_focuses={a b=yes c=0} focus={current=d progress=0.25}}}')
            self.assertEqual(_request(0,'source_snapshot',path=str(path),country='HUN'),read_snapshot(path,'HUN').to_json())
            with self.assertRaisesRegex(ValueError,'Nazione AUS'):_request(0,'source_snapshot',path=str(path),country='AUS')

    def test_demo_tree_snapshot_status_and_missing_tree_are_explicit(self):
        root=Path(__file__).resolve().parents[1]/'src/hoi4companion/examples'
        snapshot=root.parents[2]/'examples/demo_snapshot.json'
        handle=_request(0,'catalog_open',settings={'game_dir':'','country':'AUS','snapshot_path':str(snapshot)},demo_file=str(root/'demo_focus.txt'),demo_labels=str(root/'demo_labels.json'))
        self.addCleanup(lambda:_request(0,'catalog_release',handle=handle))
        data=_request(0,'catalog_sources',handle=handle);tree_id=next(iter(data['trees']))
        result=_request(0,'catalog_tree',handle=handle,tree_id=tree_id,include_snapshot=True)
        self.assertEqual(result['id'],tree_id);self.assertTrue(result['has_snapshot'])
        self.assertTrue(result['status']);self.assertTrue(all('estimated_days' in f for f in result['focuses']))
        plain=_request(0,'catalog_tree',handle=handle,tree_id=tree_id,include_snapshot=False)
        self.assertIsNone(plain['snapshot']);self.assertEqual(plain['status'],{})
        self.assertIn('error',_request(0,'catalog_tree',handle=handle,tree_id='missing'))
        overview=_request(0,'catalog_overview',handle=handle)
        self.assertEqual(overview['application'],'hoi4companion');self.assertTrue(overview['demo'])
