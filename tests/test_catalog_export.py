import copy
import json
import unittest
from types import SimpleNamespace
from hoi4companion.catalog_export import for_ai
from hoi4companion.guide_importer import Importer
from hoi4companion.import_sources import acquire_source


class CatalogExportTests(unittest.TestCase):
    def test_local_labels_are_removed_but_identity_and_relations_are_unchanged(self):
        entries=[{'kind':'focus','id':'f','name':'Focus','localized_name':'Obiettivo','aliases':['Obiettivo'],
                  'scope':'tree','prerequisites':[['a']],'mutually_exclusive':['b'],'countries':['TST']},
                 {'kind':'event_option','id':'e.a','event_id':'e','name':'Accept','localized_name':'Accetta','aliases':['Accetta'],
                  'country_condition':{'op':'unknown'}}]
        before=copy.deepcopy(entries)
        result=for_ai(entries,{'F':'Focus','O':'Accept'},{'F':'Obiettivo','O':'Accetta'})
        self.assertEqual(entries,before)
        for old,new in zip(entries,result):
            self.assertNotIn('localized_name',new);self.assertNotIn('aliases',new)
            self.assertEqual(new,{k:v for k,v in old.items() if k not in ('localized_name','aliases')})
        result[0]['prerequisites'][0].append('x');self.assertEqual(entries,before)

    def test_only_english_reference_aliases_are_exported_including_national_names(self):
        row={'kind':'equipment','id':'rifle','name':'Infantry Equipment I','localized_name':'Fucili I',
             'aliases':['Fucili I','Karabiner 98k','Fucile Tipo 99','infantry equipment 1','Unknown mod name'],
             'country_aliases':{'GER':['Karabiner 98k'],'JAP':['Type 99','Fucile Tipo 99']},
             'equipment_ids':['rifle'],'family_id':'infantry_equipment'}
        refs={'R':'Infantry Equipment I','GER_R':'Karabiner 98k','JAP_R':'Type 99'}
        local={'R':'Fucili I','JAP_R':'Fucile Tipo 99'}
        out=for_ai([row],refs,local)[0]
        self.assertEqual(out['aliases'],['Karabiner 98k'])
        self.assertEqual(out['country_aliases'],{'GER':['Karabiner 98k'],'JAP':['Type 99']})
        self.assertEqual(out['equipment_ids'],['rifle'])

    def test_english_aliases_equal_to_the_primary_name_are_not_repeated(self):
        row={'kind':'technology','id':'tools','name':'Basic Tools','localized_name':'Strumenti di base',
             'aliases':['Basic Tools',' BASIC   TOOLS ','Machine Tools','Nome sconosciuto']}
        refs={'A':'Basic Tools','B':' BASIC   TOOLS ','C':'Machine Tools'}
        self.assertEqual(for_ai([row],refs)[0]['aliases'],['Machine Tools'])
        self.assertNotIn('aliases',for_ai([row])[0])

    def test_shared_proper_names_and_local_only_mod_fallback_are_not_lost(self):
        row={'kind':'state','id':'64','name':'STATE_64','localized_name':'Regione mod','aliases':['Regione mod']}
        out=for_ai([row],{}, {'STATE_64':'Regione mod'})[0]
        self.assertEqual(out['fallback_name'],'Regione mod')
        shared={'kind':'equipment','id':'r','name':'Rifles','localized_name':'Fucili','aliases':['Garand']}
        self.assertEqual(for_ai([shared],{'A':'Garand'},{'A':'Garand'})[0]['aliases'],['Garand'])

    def test_indirect_english_labels_are_not_misclassified_as_local_translations(self):
        row={'kind':'equipment','id':'tank','name':'Tank','localized_name':'Carro','aliases':['Tank Designer']}
        reference={'A':'$B$ Designer','B':'Tank'}
        self.assertEqual(for_ai([row],reference,{'A':'Tank Designer'})[0]['aliases'],['Tank Designer'])

    def test_packet_uses_full_aliases_to_select_sources_then_exports_without_localisation(self):
        rows=[{'kind':'law','id':'law','name':'War Economy','localized_name':'Economia di guerra','aliases':['Economia di guerra'],'group':'economy'}]
        src=acquire_source(text='Adotta Economia di guerra.')
        importer=Importer(lambda:rows,catalog_export=lambda entries:for_ai(entries,{'L':'War Economy'},{'L':'Economia di guerra'}))
        packet=importer.packet([src])
        self.assertEqual(packet['catalog'][0]['id'],'law');self.assertNotIn('localized_name',packet['catalog'][0])
        self.assertEqual(rows[0]['localized_name'],'Economia di guerra')
        self.assertTrue(importer.catalog()[0]['aliases'])

    def test_localized_input_still_resolves_through_the_internal_catalog(self):
        rows=[{'kind':'building','id':'arms_factory','name':'Military Factory','localized_name':'Fabbrica militare','aliases':['Fabbrica militare']},
              {'kind':'state','id':'55','name':'Example State','localized_name':'Regione esempio','aliases':['Regione esempio']}]
        importer=Importer(lambda:rows)
        compact=importer.ai_catalog();self.assertTrue(all('localized_name' not in e for e in compact))
        text='Costruisci due fabbriche militari nella Regione esempio.'
        value=importer.document({'format':'hoi4-guide-authoring','authoring_schema_version':1,
            'sources':[{'id':'s','type':'manual','text':text,'segments':[]}],
            'guide':{'id':'localized','title':'Localized','steps':[{'key':'build','type':'build','building':'Fabbrica militare',
                'state':'Regione esempio','quantity':{'mode':'add','value':2},'evidence':[{'source_id':'s','basis':'explicit','quote':text}]}]}})
        result=importer.preview(value['guide'],value['sources'])
        self.assertTrue(result['valid'],result['errors'])
        params=result['guide']['tracks']['operations'][0]['action']['params']
        self.assertEqual((params['building_id'],params['state_id']),('arms_factory',55))
        self.assertEqual(result['operations'][0]['monitoring'],'automatic')

    def test_http_normal_catalog_remains_localized_and_export_is_explicit(self):
        from http.server import ThreadingHTTPServer
        from urllib.request import urlopen
        from urllib.error import HTTPError
        from threading import Thread
        from hoi4companion.api import make_handler
        rows=[{'kind':'building','id':'arms_factory','name':'Military Factory','localized_name':'Fabbrica militare','aliases':['Fabbrica militare']}]
        importer=Importer(lambda:rows)
        with ThreadingHTTPServer(('127.0.0.1',0),make_handler(SimpleNamespace(importer=lambda:importer,demo=False))) as server:
            t=Thread(target=server.serve_forever,daemon=True);t.start()
            try:
                base=f'http://127.0.0.1:{server.server_port}/api/import/catalog'
                with urlopen(base) as r:self.assertIn('localized_name',json.load(r)['entities'][0])
                with urlopen(base+'?export=ai') as r:
                    value=json.load(r);self.assertNotIn('localized_name',value['entities'][0]);self.assertEqual(value['localization_policy'],'local_lookup')
                    self.assertEqual(value['alias_policy'],'english_reference_only')
                with self.assertRaises(HTTPError) as error:urlopen(base+'?export=unknown')
                self.assertEqual(error.exception.code,400)
            finally:server.shutdown();t.join(timeout=3)


if __name__=='__main__':unittest.main()
