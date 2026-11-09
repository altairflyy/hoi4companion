import unittest
from test_country import CountryFixture
from hoi4companion.country_native import read_section
from hoi4companion._country_reference import CountryReader as Reference


class BackendCountryTests(unittest.TestCase):
    def test_action_event_foreign_and_province_values_keep_exact_watch_evidence(self):
        from hoi4companion.country import CountryReader
        from test_politics_native import PoliticsFixture
        from test_action_resources import ActionResourceTests
        from test_events_native import EventFixture
        from test_foreign_interactions import ForeignNativeTests
        from test_fortifications import ProvincialLiveTests
        cases=[('politics',PoliticsFixture()),('projects',ActionResourceTests().project_fixture(19999)[0]),
               ('events',EventFixture()),('intelligence',ForeignNativeTests().fixture()[0]),
               ('fortifications',ProvincialLiveTests().fixture())]
        for section,memory in cases:
            with self.subTest(section=section):
                old=Reference(memory,memory.world,memory.country,1)
                new=CountryReader(memory,memory.world,memory.country,1)
                self.assertEqual(getattr(new,section)(),getattr(old,section)())
                new.s.verify();self.assertEqual(new.watch,old.watch)

    def test_production_and_complete_design_match_independent_oracles(self):
        from test_design_native import DesignNativeTests
        from hoi4companion._design_reference import read_design as reference_design
        for qualified in (False,True):
            memory=CountryFixture()
            if qualified:
                from hoi4companion.live import MILITARY_HASH
                memory.profile.sha256=MILITARY_HASH
                memory.profile.bytes=lambda r,n:b'\0'*n
            old=Reference(memory,memory.world,memory.country,1);new=memory.reader()
            self.assertEqual(new.production(),old.production());self.assertEqual(new.watch,old.watch)
        fixture=DesignNativeTests().fixture()
        memory,variant=fixture[:2]
        old=Reference(memory,memory.world,memory.country,1);new=Reference(memory,memory.world,memory.country,1)
        from hoi4companion.design_native import read_design
        self.assertEqual(read_design(new,variant),reference_design(old,variant))
        self.assertEqual(new.watch,old.watch)

    def test_military_metrics_and_doctrine_values_and_witnesses_match(self):
        from unittest.mock import patch
        from test_military_native import MilitaryNativeTests
        from test_country_metrics_native import CountryMetricsNativeTests
        from test_doctrines_native import DoctrineFixture
        from hoi4companion import _military_native_reference as military
        from hoi4companion import _country_metrics_native_reference as metrics
        from hoi4companion import _doctrines_native_reference as doctrines
        import hashlib
        from hoi4companion.country import CountryReader
        memory=MilitaryNativeTests().fixture()[0]
        for section in ('experience','diplomacy'):
            old=Reference(memory,memory.world,memory.country,1);new=CountryReader(memory,memory.world,memory.country,1)
            self.assertEqual(getattr(new,section)(),getattr(military,'read_'+section)(old));self.assertEqual(new.watch,old.watch)
        signatures=((0x100,3,hashlib.sha256(b'abc').hexdigest()),)
        with patch('hoi4companion.country_metrics_native._FUNCTIONS',signatures),patch.object(metrics,'_FUNCTIONS',signatures):
            memory=CountryMetricsNativeTests().fixture()[0]
            old=Reference(memory,memory.world,memory.country,1);new=CountryReader(memory,memory.world,memory.country,1)
            self.assertEqual(new.metrics(),metrics.read_metrics(old));self.assertEqual(new.watch,old.watch)
        signatures=tuple((r,n,hashlib.sha256(b'\x90'*n).hexdigest()) for r,n,_ in doctrines._FUNCTIONS)
        with patch('hoi4companion.doctrines_native._FUNCTIONS',signatures),patch.object(doctrines,'_FUNCTIONS',signatures):
            memory=DoctrineFixture();old=Reference(memory,memory.world,memory.country,1);new=memory.reader()
            self.assertEqual(new.doctrines(),doctrines.read_doctrines(old));self.assertEqual(new.watch,old.watch)

    def test_queue_resource_and_agency_values_keep_their_evidence(self):
        from test_construction import ConstructionFixture
        from test_intelligence import IntelligenceFixture
        resources=CountryFixture();resources.resources()
        for section,memory in [('resources',resources),('construction',ConstructionFixture()),
                               ('construction',ConstructionFixture(provincial=True)),('intelligence',IntelligenceFixture())]:
            with self.subTest(section=section,fixture=type(memory).__name__):
                old=Reference(memory,memory.world,memory.country,1);new=memory.reader()
                self.assertEqual(read_section(new,section),getattr(old,section)())
                new.s.verify();self.assertEqual(new.watch,old.watch)

    def test_native_sections_and_watch_evidence_match_reference(self):
        for section in ('divisions','manpower','states','equipment'):
            for legacy_name in (False,True):
                with self.subTest(section=section,legacy_name=legacy_name):
                    memory=CountryFixture()
                    if section=='equipment':memory.stockpile(amount=9_999_999_999_999_999)
                    if legacy_name:
                        pointer=int.from_bytes(memory.read(memory.template+0x20,8),'little')
                        memory.put(pointer,'<B',0x81)
                    old=Reference(memory,memory.world,memory.country,1);new=memory.reader()
                    expected=getattr(old,section)();actual=read_section(new,section)
                    new.s.verify()
                    self.assertEqual(actual,expected)
                    self.assertEqual(new.watch,old.watch)
