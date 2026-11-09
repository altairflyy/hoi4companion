import copy
import unittest

from hoi4companion.association_review import review_associations,review_markdown
from hoi4companion.resolution import CatalogIndex


class AssociationReviewTests(unittest.TestCase):
    def test_every_occurrence_is_kept_and_unavailable_exact_ids_are_not_enabled(self):
        entries=[{'kind':'focus','id':'old','scope':'legacy','name':'Old Focus','availability':'outside_active_catalog',
                  'availability_reason':'Requires a different DLC','source':'legacy.txt'},
                 {'kind':'focus','id':'new','scope':'active','name':'New Focus','availability':'available'}]
        index=CatalogIndex(entries)
        report={'results':[{'title':'First Guide','unresolved':[
            {'key':'a','kind':'focus','query':'Old Focus','status':'unresolved','context':{},'candidates':[]},
            {'key':'b','kind':'focus','query':'Unknown Completely','status':'unresolved','context':{},'candidates':[]}]},
            {'title':'Second Guide','unresolved':[{'key':'a','kind':'focus','query':'Old Focus','status':'unresolved','context':{},'candidates':[]}]}]}
        before=copy.deepcopy(report)
        review=review_associations(report,index)
        self.assertEqual(len(review['results']),1)
        old=review['catalog_compatibility'][0]
        self.assertEqual(old['status'],'documented_identity_outside_active_catalog')
        self.assertEqual(len(old['occurrences']),2)
        self.assertEqual(old['candidates'][0]['id'],'old')
        self.assertEqual(review['results'][0]['status'],'no_supported_candidate')
        self.assertEqual(report,before)
        markdown=review_markdown(review)
        self.assertIn('Altre incompatibilità di catalogo',markdown)
        self.assertIn('`old`',markdown)
        self.assertIn('Nessuna guida è stata modificata',markdown)

    def test_dlc_blocked_guides_do_not_enter_any_review_list(self):
        index=CatalogIndex([])
        report={'results':[{'title':'DLC Guide','compatibility':{'blocked':True,'requirements':[{'enable':['Pack'],'disable':[]}]},
            'errors':['step: unresolved focus','DLC requirement not met'],
            'unresolved':[{'key':'step','kind':'focus','query':'Focus','status':'unresolved','candidates':[]}]}]}
        review=review_associations(report,index)
        self.assertEqual(review['results'],[])
        self.assertEqual(review['other_import_errors'],[])
        self.assertNotIn('unavailable_guides',review)
        markdown=review_markdown(review)
        self.assertNotIn('DLC Guide',markdown)
        self.assertNotIn('Pack',markdown)
        self.assertNotIn('DLC',markdown)
        self.assertIn('Nessuna associazione da confermare',markdown)

    def test_same_name_in_distinct_contexts_is_not_collapsed_or_silently_selected(self):
        index=CatalogIndex([{'kind':'focus','id':'a','name':'Common Focus','scope':'A','availability':'available'},
                            {'kind':'focus','id':'b','name':'Common Focus','scope':'B','availability':'available'}])
        report={'results':[{'title':'Guide','unresolved':[
            {'key':str(i),'kind':'focus','query':'Common Focus','status':'ambiguous','context':{'scope':scope},'candidates':[]}
            for i,scope in enumerate(['A','B'])]}]}
        review=review_associations(report,index)
        self.assertEqual(len(review['results']),2)
        for row in review['results']:
            self.assertEqual(len(row['candidates']),2)
            self.assertEqual(row['status'],'documented_identity_needs_context')
        markdown=review_markdown(review)
        self.assertIn('Più ID possibili: serve scegliere il ramo o il livello.',markdown)
        self.assertNotIn('nome/alias documentato; non selezionato automaticamente',markdown)
        self.assertNotIn('Ambito: {',markdown)
