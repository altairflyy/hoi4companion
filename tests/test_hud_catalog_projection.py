import copy,unittest
from hoi4companion.screen_overlay import targets
import test_screen_overlay as fixture


class CountedCatalog(list):
    def __iter__(self):
        self.scans+=1
        return super().__iter__()


class HudCatalogProjectionTests(unittest.TestCase):
    def test_large_unrelated_catalog_is_not_rescanned_per_highlight(self):
        f=fixture.ScreenOverlayTests();f.setUp()
        second=copy.deepcopy(f.guide['tracks']['focus'][0]);second['key']='f2';second['action']['entity']['id']='other_focus'
        f.guide['tracks']['focus'].append(second)
        f.runtime['parallel']['focus']['steps'].append('f2');f.runtime['steps'].append({'key':'f2','status':'ready'})
        f.catalog.append({**f.catalog[0],'id':'other_focus','name':'Other focus'})
        plan={'guide_id':f.guide['id'],'bindings':{}}
        expected=targets(f.guide,f.runtime,f.catalog,f.state,event_plan=plan)
        rows=CountedCatalog([{'kind':'focus','id':'unused-'+str(i),'name':'Unused'} for i in range(3000)]+f.catalog)
        rows.scans=0
        self.assertEqual(targets(f.guide,f.runtime,rows,f.state,event_plan=plan),expected)
        self.assertLessEqual(rows.scans,3)

    def test_catalog_changes_are_visible_in_the_next_projection(self):
        f=fixture.ScreenOverlayTests();f.setUp();plan={'guide_id':f.guide['id'],'bindings':{}}
        before=targets(f.guide,f.runtime,f.catalog,f.state,event_plan=plan)
        for row in f.catalog:
            row['name']='Changed name';row['localized_name']='Changed label'
        after=targets(f.guide,f.runtime,f.catalog,f.state,event_plan=plan)
        self.assertEqual([x['identity'] for x in after],[x['identity'] for x in before])
        self.assertNotEqual([x['labels'] for x in after],[x['labels'] for x in before])
