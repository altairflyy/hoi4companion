import io
import unittest
import zipfile
from xml.sax.saxutils import escape

from hoi4companion.word_guides import list_documents, prepare_document
from hoi4companion.guide_authoring import compile_document
from hoi4companion.guide_importer import preview
from hoi4companion.guide_runtime import evaluate


def word(cells):
    def cell(lines):
        return '<w:tc>' + ''.join('<w:p><w:r><w:t>' + escape(line) + '</w:t></w:r></w:p>' for line in lines) + '</w:tc>'
    xml = ('<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>'
           '<w:tbl><w:tr>' + ''.join(cell([heading]) for heading in ['ACTIONS', 'RESEARCH PRIORITY', 'FOCUS PRIORITY']) + '</w:tr>'
           '<w:tr>' + ''.join(cell(lines) for lines in cells) + '</w:tr></w:tbl>'
           '<w:p><w:r><w:t>WAR PLANS: prepare the western front.</w:t></w:r></w:p></w:body></w:document>')
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'w', zipfile.ZIP_DEFLATED) as z:
        z.writestr('word/document.xml', xml)
    return buf.getvalue()


class WordGuideTests(unittest.TestCase):
    def test_parenthesized_no_focus_instruction_is_not_an_unreadable_focus_slot(self):
        prepared=prepare_document(io.BytesIO(word([[],[],['A','(No Focus until 150PP)','Z']])))
        steps=prepared['document']['guide']['steps']
        self.assertEqual([s['type'] for s in steps],['focus','reminder','focus'])
        self.assertEqual(steps[1]['evidence'][0]['quote'],'(No Focus until 150PP)')

    def test_unreadable_parenthesized_focus_retains_its_slot_and_source_evidence(self):
        prepared=prepare_document(io.BytesIO(word([[],[],['A','(???)','Z']])))
        compiled=compile_document(prepared['document'])
        g=compiled['guide'];g['applies_to']={'tree_id':'tree'}
        g['focus_sequence']={'schema_version':1,'coverage':'complete','initial_focuses':[]}
        entries=[{'kind':'focus','id':k,'name':k,'scope':'tree','prerequisites':p,
                  'mutually_exclusive':[],'bypass_possible':False,'sequence_effects_safe':True}
                 for k,p in [('A',[]),('B',[['A']]),('Z',[['B']])]]
        r=preview(g,prepared['sources'],entries)
        self.assertTrue(r['valid'],r['errors'])
        rows=r['guide']['tracks']['focus']
        self.assertEqual([x['target'] for x in rows],['A','B','Z'])
        self.assertEqual(rows[1]['focus_resolution']['position'],2)
        self.assertEqual(rows[1]['evidence'][0]['quote'],'(???)')

    def test_category_and_explicit_optional_alternatives_remain_instructions(self):
        lines=['Air Techs','Industry Upgrades','Support Ships / Feel free to pick another one or none (Special Project)']
        steps=prepare_document(io.BytesIO(word([[],lines,[]])))['document']['guide']['steps']
        self.assertTrue(all(s['type']=='reminder' for s in steps))
        self.assertEqual([s['instructions'][0] for s in steps],lines)
    def test_balanced_editorial_annotations_do_not_become_part_of_native_name(self):
        lines=['To War Plan Orange', 'ASAP: Power Struggle',
               '(No earlier than Jan 1st) A New Expedition',
               'Fair Labor Standard Acts(on March 16th, special measures(50PP) if needed)',
               'Demand Slovakia (force deploy if needed)*']
        result=prepare_document(io.BytesIO(word([[],[],lines])))
        steps=result['document']['guide']['steps']
        self.assertEqual([s['target'] for s in steps],['To War Plan Orange','Power Struggle',
            'A New Expedition','Fair Labor Standard Acts','Demand Slovakia'])
        for step,line in zip(steps,lines):
            self.assertEqual(step['evidence'][0]['quote'],line)
            if step.get('target')!=line:self.assertEqual(step['note'],line)
    def test_editorial_after_parenthetical_remains_free_or_reminder(self):
        result=prepare_document(io.BytesIO(word([[],[],[
            'Free (rest of game)','Nothing (unless needed)', 'Air Production/Free', 'Suggested:']])))
        self.assertEqual([s['type'] for s in result['document']['guide']['steps']],
                         ['free','reminder','free','reminder'])
    def test_editorial_headings_and_free_choice_variations_are_not_native_entities(self):
        payload=word([[],['Air Techs/Free','Free*','Industry/free'],['My suggestions:','Nothing','Free*']])
        steps=prepare_document(io.BytesIO(payload))['document']['guide']['steps']
        self.assertEqual([s['type'] for s in steps],['free','free','free','reminder','reminder','free'])
        self.assertEqual(steps[0]['note'],'Air Techs/Free')
    def setUp(self):
        self.doc = word([['Build MILs in Berlin', 'On war: prepare front'],
                         ['Basic Machine Tools', 'Artillery UP'], ['The Four Year Plan', 'Next Focus (if available)']])
        self.archive = io.BytesIO()
        with zipfile.ZipFile(self.archive, 'w', zipfile.ZIP_DEFLATED) as z:
            z.writestr('Country Guides/Poland.docx', self.doc)
            z.writestr('Tools/ABBREVIATIONS & TOOLS.docx', word([['MIL means factory'], [], []]))
            z.writestr('designs/DEF.png', b'not an image')

    def test_archive_lists_only_documents_and_prepares_selected_source_and_reviewable_draft(self):
        self.archive.seek(0)
        self.assertEqual(len(list_documents(self.archive)['documents']), 2)
        self.archive.seek(0)
        result = prepare_document(self.archive, 'Country Guides/Poland.docx')
        source, draft = result['sources'][0], result['document']
        self.assertIn('Build MILs in Berlin', source['text'])
        self.assertIn('WAR PLANS: prepare the western front.', source['text'])
        steps = draft['guide']['steps']
        self.assertEqual([s['target'] for s in steps if s['type'] == 'research'], ['Basic Machine Tools', 'Artillery UP'])
        self.assertEqual([s['target'] for s in steps if s['type'] == 'focus'], ['The Four Year Plan', 'Next Focus'])
        self.assertTrue(any(s['type'] == 'reminder' and 'Build MILs' in s['instructions'][0] for s in steps))
        reminder = next(s for s in steps if 'Build MILs' in s.get('title',''))
        self.assertIn('MIL = military factory', reminder['note'])
        self.assertEqual(draft['guide']['dependency_policy'], 'explicit')
        for step in steps:
            self.assertIn(step['evidence'][0]['quote'], source['text'])
        compiled = compile_document(draft)
        self.assertEqual(len(compiled['guide']['tracks']['research']), 2)

    def test_ambiguous_missing_structure_stays_text_only_and_bad_archive_entries_are_rejected(self):
        payload = word([['do something'], [], []])
        with self.assertRaises(ValueError):
            prepare_document(io.BytesIO(self.archive.getvalue()), '../outside.docx')
        result = prepare_document(io.BytesIO(payload), None)
        self.assertTrue(result['sources'][0]['text'])

    def test_optional_glossary_stays_separate_and_does_not_invent_actions(self):
        self.archive.seek(0)
        result = prepare_document(self.archive, 'Country Guides/Poland.docx', 'Tools/ABBREVIATIONS & TOOLS.docx')
        self.assertEqual(len(result['sources']), 2)
        self.assertIn('MIL means factory', result['sources'][1]['text'])
        self.assertEqual(result['document']['sources'], result['sources'])
        self.assertFalse(any('MIL means factory' in str(step) for step in result['document']['guide']['steps']))

    def test_hyperlink_targets_are_retained_as_passive_source_text(self):
        buf = io.BytesIO()
        with zipfile.ZipFile(io.BytesIO(self.doc)) as original, zipfile.ZipFile(buf, 'w') as z:
            z.writestr('word/document.xml', original.read('word/document.xml'))
            z.writestr('word/_rels/document.xml.rels', '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/video" TargetMode="External"/></Relationships>')
        result = prepare_document(io.BytesIO(buf.getvalue()))
        self.assertIn('https://example.com/video', result['sources'][0]['text'])

    def test_free_choices_and_project_annotations_are_not_mistaken_for_ids(self):
        payload = word([[], ['Flamethrower Tank (Special Project) (Get Scientist) (Might be later)',
                              'Special Project: Military Engineering Vehicles', 'Free/Air Techs'],
                             ['Free/Air Oriented Ones']])
        steps = prepare_document(io.BytesIO(payload))['document']['guide']['steps']
        self.assertEqual([(s['type'],s.get('target')) for s in steps[:2]],
                         [('special_project','Flamethrower Tank'),('special_project','Military Engineering Vehicles')])
        self.assertEqual([(s['type'],s.get('lane')) for s in steps[2:]], [('free','research'),('free','focus')])
        self.assertIn('Get Scientist',steps[0]['note'])
        self.assertEqual(steps[2]['note'],'Free/Air Techs')

    def test_research_original_title_survives_binding_for_later_up_steps(self):
        payload = word([[], ['Infantry Equip UP','Infantry Equip UP'], []])
        steps = prepare_document(io.BytesIO(payload))['document']['guide']['steps']
        self.assertEqual(steps[0]['title'], 'Infantry Equip UP')

    def test_explicit_standalone_laws_and_agency_are_actions_not_reminders(self):
        payload=word([['War Economy (150PP)','- Limited Conscription (150PP)',
                       'Create Agency','On War: War Economy (150PP)',
                       'War Economy (150PP) and recruit divisions'],[],[]])
        steps=prepare_document(io.BytesIO(payload))['document']['guide']['steps']
        self.assertEqual([s['type'] for s in steps],['law','law','create_agency','reminder','reminder'])
        self.assertEqual(steps[0]['target'],'War Economy')
        self.assertEqual(steps[0]['political_power_cost'],150)
        self.assertEqual(steps[1]['target'],'Limited Conscription')
        self.assertTrue(all(s['evidence'][0]['quote'] in prepare_document(io.BytesIO(payload))['sources'][0]['text'] for s in steps))

    def test_imported_law_progress_requires_observed_law_not_just_mentioned_text(self):
        payload=word([['War Economy (150PP)'],[],[]]);doc=prepare_document(io.BytesIO(payload))['document']
        compiled=compile_document(doc)
        catalog=[{'kind':'law','id':'war_economy','name':'War Economy','group':'economy','aliases':[]}]
        report=preview(compiled['guide'],doc['sources'],catalog)
        row=report['guide']['tracks']['operations'][0]
        self.assertEqual(row['action']['params']['law_id'],'war_economy')
        fact=lambda value:{'status':'observed','source':'fixture','value':value}
        known={'schema_version':1,'usable':True,'facts':{'country':fact('TST'),'politics.ideas':fact(['war_economy'])}}
        self.assertEqual(evaluate(report['guide'],known)['steps'][0]['status'],'completed')
        self.assertNotEqual(evaluate(report['guide'],{'schema_version':1,'usable':True,'facts':{}})['steps'][0]['status'],'completed')

    def test_named_event_and_choice_bind_to_catalog_but_not_automatic_completion(self):
        payload=word([['Event: The Test Event; choose: A Test Choice'],[],[]])
        document=prepare_document(io.BytesIO(payload))['document']
        step=document['guide']['steps'][0]
        self.assertEqual(step['type'],'event')
        self.assertEqual((step['event'],step['option']),('The Test Event','A Test Choice'))
        catalog=[{'kind':'event','id':'test.1','name':'The Test Event'},
                 {'kind':'event_option','id':'test.1.a','event_id':'test.1','name':'A Test Choice'}]
        compiled=compile_document(document)
        result=preview(compiled['guide'],document['sources'],catalog)
        row=result['guide']['tracks']['operations'][0]
        self.assertEqual(row['action']['params']['event_id'],'test.1')
        self.assertEqual(row['action']['params']['event_option_id'],'test.1.a')
        self.assertIn('started_when',row)
        self.assertNotIn('completion',row)

    def test_exact_construction_quantity_can_be_tracked_without_guessing(self):
        payload=word([['Build 2 military factories in Berlin',
                       'Build MILs in Berlin', 'Build 2 military factories in Berlin and train'],[],[]])
        document=prepare_document(io.BytesIO(payload))['document']
        steps=document['guide']['steps']
        self.assertEqual([s['type'] for s in steps],['build','reminder','reminder'])
        self.assertEqual(steps[0]['quantity'],{'mode':'add','value':2})
        catalog=[{'kind':'building','id':'arms_factory','name':'Military Factory','aliases':[]},
                 {'kind':'state','id':'64','name':'Berlin','aliases':[]}]
        compiled=compile_document(document);report=preview(compiled['guide'],document['sources'],catalog)
        self.assertTrue(report['valid'],report['errors'])
        step=report['guide']['tracks']['operations'][0]
        self.assertEqual(step['action']['params']['state_id'],64)
        self.assertIn('started_when',step);self.assertIn('completion',step)

    def test_project_annotations_are_retained_but_not_part_of_project_name(self):
        payload=word([[],['Military Engineering Vehicle (Special Project) (hire scientist) (if possible)',
                          'Flame Tanks (Special Project) (Get Scientist (25PP))',
                          'Radar (Special Project) (Optional)'],[]])
        steps=prepare_document(io.BytesIO(payload))['document']['guide']['steps']
        self.assertEqual([s['target'] for s in steps],['Military Engineering Vehicle','Flamethrower Tank','Radar'])
        self.assertIn('25PP',steps[1]['note'])
        catalog=[{'kind':'project','id':'flame','name':'Flamethrower Tank','aliases':[]}]
        single=word([[],['Flame Tanks (Special Project) (Get Scientist (25PP))'],[]])
        doc=prepare_document(io.BytesIO(single))['document'];compiled=compile_document(doc)
        result=preview(compiled['guide'],doc['sources'],catalog)
        self.assertTrue(result['valid'],result['errors'])
        self.assertEqual(result['guide']['tracks']['operations'][0]['action']['entity']['id'],'flame')

    def test_combined_project_annotations_and_asap_are_not_part_of_identity(self):
        payload=word([[],['Military Engineering Vehicles (Special Project, assign scientist)',
                          'ASAP: Flame Tank (Special Project)',
                          'Military Engineering Vehicles (Special Project) (Recruit Scientist, Optional)'],[]])
        steps=prepare_document(io.BytesIO(payload))['document']['guide']['steps']
        self.assertEqual([s['target'] for s in steps],['Military Engineering Vehicles','Flamethrower Tank','Military Engineering Vehicles'])

