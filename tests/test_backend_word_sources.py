"""Word content is passive source data; archive paths are never extracted."""
import io,json,zipfile
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from hoi4companion._word_guides_reference import list_documents,prepare_document
from hoi4companion.backend_core import _request
from test_word_guides import word

class BackendWordSourcesTests(unittest.TestCase):
    def test_reading_hints_follow_the_supplied_vocabulary(self):
        from copy import deepcopy
        from hoi4companion.research_upgrades import guide_language
        vocabulary=deepcopy(guide_language());vocabulary['abbreviations']['construction']['ZX']='fixture structure'
        with TemporaryDirectory() as folder:
            path=Path(folder)/'guide.docx';path.write_bytes(word([['Build ZX'],[],[]]))
            packet=_request(0,'word_prepare',path=str(path),vocabulary=vocabulary)
            self.assertIn('ZX = fixture structure',packet['document']['guide']['steps'][0]['note'])
    def test_priority_actions_free_slots_projects_and_source_ordinals_match(self):
        data=word([['War Economy (150PP)','Build MILs in Berlin','Build 2 military factories in Berlin','Create Agency','Event: Test; choose: Accept'],
                   ['Artillery UP','Flame Tanks (Special Project) (Get Scientist (25PP))','Free/Air Techs'],
                   ['(No earlier than Jan 1st) A New Expedition','Fair Labor Standard Acts(special measures(50PP))','(???)','(No Focus until 150PP)']])
        with TemporaryDirectory() as folder:
            path=Path(folder)/'guide.docx';path.write_bytes(data)
            self.assertEqual(_request(0,'word_documents',path=str(path)),list_documents(io.BytesIO(data)))
            self.assertEqual(_request(0,'word_prepare',path=str(path)),prepare_document(io.BytesIO(data)))

    def test_collection_and_context_are_bounded_separate_sources_with_safe_names(self):
        data=io.BytesIO()
        with zipfile.ZipFile(data,'w') as archive:
            archive.writestr('Country Guides/Guide.docx',word([['Create Agency'],[],['Focus']]))
            archive.writestr('Tools/Abbreviations.docx',word([['MIL means factory'],[],[]]))
            archive.writestr('../unsafe.docx',b'unsafe')
        with TemporaryDirectory() as folder:
            path=Path(folder)/'collection.zip';path.write_bytes(data.getvalue())
            self.assertEqual(_request(0,'word_documents',path=str(path)),list_documents(io.BytesIO(data.getvalue())))
            self.assertEqual(_request(0,'word_prepare',path=str(path),entry='Country Guides/Guide.docx',context='Tools/Abbreviations.docx'),prepare_document(io.BytesIO(data.getvalue()),'Country Guides/Guide.docx','Tools/Abbreviations.docx'))
            with self.assertRaises(ValueError):_request(0,'word_prepare',path=str(path),entry='../unsafe.docx')

    def test_duplicate_central_directory_names_and_xml_entities_are_rejected(self):
        with TemporaryDirectory() as folder:
            path=Path(folder)/'bad.docx'
            with zipfile.ZipFile(path,'w') as archive:
                archive.writestr('word/document.xml','<!DOCTYPE data [<!ENTITY a "text">]><data>&a;</data>')
            with self.assertRaises(ValueError):_request(0,'word_prepare',path=str(path))
            with zipfile.ZipFile(path,'w') as archive:
                archive.writestr('word/document.xml','<data/>');archive.writestr('word/document.xml','<data/>')
            with self.assertRaises(ValueError):_request(0,'word_prepare',path=str(path))
