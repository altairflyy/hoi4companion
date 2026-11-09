import io
import unittest

from hoi4companion.guide_authoring import compile_document
from hoi4companion.guide_importer import preview
from hoi4companion.resolution import CatalogIndex, ResolutionContext
from hoi4companion.word_guides import prepare_document
from test_word_guides import word


def focus(identifier, name, parents=(), mutex=(), **extra):
    return dict(kind='focus', id=identifier, name=name, scope='tree', countries=['AAA'],
                prerequisites=list(parents), mutually_exclusive=list(mutex),
                bypass_possible=False, external_completion_possible=False, **extra)


class ZipResolutionTests(unittest.TestCase):
    def test_dlc_unavailability_is_not_counted_as_an_import_review(self):
        from hoi4companion.guide_importer import Importer
        from test_guide_importer import definition,source
        src=source();g=definition(src)
        missing={'status':'dlc_incompatible','blocked':True,'requirements':[{'enable':['Pack'],'disable':[]}]}
        importer=Importer(lambda:[],compatibility=lambda guide:missing)
        result=importer.preview(g,[src])
        self.assertEqual(result['availability']['status'],'dlc_unavailable')
        self.assertEqual(result['availability']['issue_count'],0)
        self.assertFalse(result['availability']['requires_review'])
        self.assertTrue(result['errors'])  # Technical detail is retained, not falsified.
        with self.assertRaisesRegex(ValueError,'DLC'):
            importer.install(g,[src],reviewed=True)
        malformed={**g,'schema_version':999}
        self.assertEqual(importer.preview(malformed,[src])['availability']['status'],'needs_revision')

    def test_scoped_compound_spelling_and_inflections_are_resolved_without_an_alias(self):
        index = CatalogIndex([focus('proposal', 'The Blum-Viollette Proposal'),
                              focus('act', 'Fair Labor Standards Act')])
        context = ResolutionContext(country='AAA', scope='tree')
        self.assertEqual([r['id'] for r in index.matches('The Bloom Violette Proposal', ('focus',), context, typos=True)], ['proposal'])
        self.assertEqual([r['id'] for r in index.matches('Fair Labor Standard Acts', ('focus',), context, typos=True)], ['act'])
        self.assertEqual(index.matches('The Bloom Violette Proposal', ('focus',), typos=True), [])
        self.assertEqual(index.matches('The_Bloom_Violette_Proposal', ('focus',), context, typos=True), [])

    def test_spelling_tolerance_keeps_ambiguity_and_protected_gameplay_words(self):
        index = CatalogIndex([focus('a', 'The Blum-Viollette Proposal'), focus('b', 'The Bloom-Viollete Proposal'),
                              focus('level', 'Advanced Heavy Tank 2')])
        context = ResolutionContext(scope='tree')
        self.assertEqual(index.matches('Basic Heavy Tank 2', ('focus',), context, typos=True), [])
        self.assertEqual(index.matches('Advanced Heavy Tank 3', ('focus',), context, typos=True), [])
        # Exact/closer spelling still precedes the larger compound budget.
        self.assertEqual([r['id'] for r in index.matches('The Bloom Violette Proposal', ('focus',), context, typos=True)], ['b'])

    def report(self, names, entries, text_note=None):
        packet=prepare_document(io.BytesIO(word([[], [], names])))
        compiled=compile_document(packet['document'])
        compiled['guide']['applies_to']={'country':'AAA', 'tree_id':'tree'}
        if text_note is not None:
            compiled['guide']['tracks']['focus'][0]['note']=text_note
        return preview(compiled['guide'], packet['sources'], entries)

    def catalog(self):
        a=focus('a', 'First Route', mutex=['b'])
        b=focus('b', 'Second Route', mutex=['a'])
        bridge=focus('bridge', 'Unrequested Bridge', [['b']]);bridge['bypass_possible']=True
        return [a,b,bridge,focus('parent', 'First Route Advance', [['a']]),
                focus('other', 'Second Route Advance', [['bridge']]),
                focus('choice-a', 'Shared Objective', [['parent']]),
                focus('choice-b', 'Shared Objective', [['other']])]

    def test_word_same_name_variant_uses_the_explicit_route_not_an_unrequested_bypass(self):
        report=self.report(['First Route', 'First Route Advance', 'Shared Objective'], self.catalog())
        self.assertTrue(report['valid'], report['errors'])
        choice=report['resolutions'][-1]
        self.assertEqual(choice['entity']['id'], 'choice-a')
        self.assertEqual(choice['association_evidence']['method'], 'declared_source_branch')
        self.assertEqual(report['guide']['tracks']['focus'][-1]['focus_resolution']['classification'], 'reconstructed')
        self.assertEqual(report['focus_sequence']['reconstructed_count'], 1)
        self.assertNotIn('completion', report['guide']['tracks']['focus'][-1])

    def test_source_branch_does_not_choose_between_two_normally_compatible_routes(self):
        entries=self.catalog();entries[0]['mutually_exclusive']=[];entries[1]['mutually_exclusive']=[]
        report=self.report(['First Route','First Route Advance','Shared Objective'],entries)
        self.assertFalse(report['valid']);self.assertEqual(report['resolutions'][-1]['status'],'ambiguous')

    def test_explicit_bypass_or_missing_graph_prevents_the_source_route_fallback(self):
        report=self.report(['First Route','First Route Advance','Shared Objective'],self.catalog(),text_note='Bypass the other branch')
        self.assertFalse(report['valid'])
        entries=self.catalog();entries=[r for r in entries if r['id']!='b']
        self.assertFalse(self.report(['First Route','First Route Advance','Shared Objective'],entries)['valid'])

    def test_a_predecessor_resolved_by_route_propagation_supports_the_next_shared_label(self):
        entries=self.catalog()
        entries[4]['name']='First Route Advance'
        entries[4]['prerequisites']=[['b']]
        entries[-1]['prerequisites']=[['other2']]
        entries.append(focus('other2','Different branch intermediate',[['bridge']]))
        # A shared predecessor is resolved by the declared first route. Its
        # anchor must remain usable even before the domain is narrowed.
        report=self.report(['First Route', 'First Route Advance', 'Shared Objective'], entries)
        self.assertTrue(report['valid'], report['errors'])
        self.assertEqual(report['resolutions'][-1]['entity']['id'],'choice-a')


if __name__ == '__main__':
    unittest.main()
