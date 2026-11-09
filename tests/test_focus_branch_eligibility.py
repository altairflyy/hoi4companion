import json
import tempfile
import unittest
from pathlib import Path

from hoi4companion.dlc import DlcInventory
from hoi4companion.eligibility import excluded_focus_branches
from hoi4companion.focus import load_trees
from hoi4companion.guide_importer import catalog_entities
from hoi4companion.resolution import CatalogIndex,ResolutionContext


class FocusBranchEligibilityTests(unittest.TestCase):
    def test_government_exclusions_only_prune_provably_incompatible_variants(self):
        from hoi4companion.clausewitz import parse
        from hoi4companion.eligibility import government_exclusions
        alternate=parse('OR={has_government=fascism has_government=neutrality}')
        self.assertEqual(government_exclusions(alternate),['communism','democratic'])
        self.assertEqual(government_exclusions(parse('unknown_trigger=yes')),[])
        normal=parse('NOT={OR={has_government=fascism has_government=neutrality}}')
        self.assertEqual(government_exclusions(normal),['fascism','neutrality'])
    def test_focus_text_override_is_the_english_identity_and_keeps_native_id(self):
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'names.txt'
            path.write_text('focus_tree={id=country focus={id=local_industry text=industrial_effort} focus={id=local_research text=extra_tech_slot}}',encoding='utf8')
            local={'industrial_effort':'Sforzo industriale','extra_tech_slot':'Slot di ricerca aggiuntivo'}
            english={'industrial_effort':'Industrial Effort','extra_tech_slot':'Extra Research Slot'}
            trees,warnings=load_trees([path],local)
        self.assertFalse(warnings)
        self.assertEqual(trees['country'].focuses['local_industry'].name,'Sforzo industriale')
        index=CatalogIndex(catalog_entities(trees,{},local,english))
        for query,expected in [('Industrial Effort','local_industry'),('Extra Research Slot','local_research')]:
            matches=index.matches(query,('focus',),ResolutionContext(scope='country'))
            self.assertEqual([r['id'] for r in matches],[expected])
    def load(self):
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'trees.txt'
            path.write_text('''focus_tree={id=test
                focus={id=old allow_branch={NOT={has_dlc=Pack} if={limit={some_rule=yes} other=yes}}}
                focus={id=old_child prerequisite={focus=old}}
                focus={id=new allow_branch={has_dlc=Pack}}
                focus={id=new_child prerequisite={focus=new}}
                focus={id=alternative prerequisite={focus=old focus=new}}
                focus={id=missing prerequisite={focus=missing_parent}}
                focus={id=runtime_only allow_branch={tag=ABC}}
                focus={id=unknown_dlc allow_branch={has_dlc=Maybe}}
                focus={id=cycle_a prerequisite={focus=cycle_b}}
                focus={id=cycle_b prerequisite={focus=cycle_a}}
            }''',encoding='utf8')
            trees,warnings=load_trees([path],{'old':'Shared Name','new':'Shared Name'})
        self.assertFalse(warnings)
        return trees

    def test_individual_dlc_gates_remove_old_variant_and_necessary_descendants_only(self):
        trees=self.load()
        before=json.dumps(trees['test'].to_json())
        dlcs=DlcInventory(items=[{'name':'Pack','status':'enabled'}],complete=False)
        excluded=excluded_focus_branches(trees,dlcs)
        self.assertEqual({e['id'] for e in excluded},{'old','old_child'})
        self.assertEqual(json.dumps(trees['test'].to_json()),before)
        rows=catalog_entities(trees,{}, {})
        excluded_ids={(e['scope'],e['id']) for e in excluded}
        index=CatalogIndex([r for r in rows if (r.get('scope'),r['id']) not in excluded_ids])
        self.assertEqual([r['id'] for r in index.matches('Shared Name',('focus',),ResolutionContext(scope='test'))],['new'])

    def test_missing_dlc_information_does_not_exclude_either_branch(self):
        self.assertEqual(excluded_focus_branches(self.load(),DlcInventory(complete=False)),[])

    def test_opposite_dlc_configuration_selects_the_legacy_variant(self):
        excluded=excluded_focus_branches(self.load(),DlcInventory(items=[{'name':'Pack','status':'disabled'}],complete=False))
        self.assertEqual({e['id'] for e in excluded},{'new','new_child'})
