"""Exact-profile political membership and ownership faults; synthetic fixtures."""
import unittest

from hoi4companion.live import MILITARY_HASH, LiveReadError, compile_watch_reads, watch_changed
from hoi4companion.politics_native import CURRENT_IDEA_SIGNATURES, CURRENT_ADVISOR_SIGNATURES, read_politics
from test_politics_native import PoliticsFixture


class CurrentPoliticsTests(unittest.TestCase):
    def fixture(self):
        f = PoliticsFixture()
        f.profile.sha256 = MILITARY_HASH
        signatures = dict(CURRENT_IDEA_SIGNATURES + CURRENT_ADVISOR_SIGNATURES)
        f.profile.bytes = lambda rva, size: signatures[rva][:size]
        f.put(f.manager + 0xC8, '<Q', f.country)
        return f

    def test_current_profile_exposes_observed_ideas_and_active_advisors(self):
        f = self.fixture()
        result = read_politics(f.reader())
        self.assertEqual(result['status'], 'partial')
        self.assertEqual(result['ideas']['status'], 'observed')
        self.assertEqual(result['ideas']['items'][0]['id'], 'limited_exports')
        self.assertEqual(result['advisors']['status'], 'observed')
        self.assertEqual(result['advisors']['items'],[{'id':'TEST_some_advisor','character_id':'TEST_some_person','slot':4}])
        self.assertEqual(result['power']['status'], 'unavailable')
        self.assertNotIn('available', result['power'])

    def test_available_roles_are_not_reported_as_active_appointments(self):
        f=self.fixture();f.array(f.characters+0x40,[f.role]);f.array(f.characters+0x58,[])
        self.assertEqual(read_politics(f.reader())['advisors'],{'status':'observed','items':[]})

    def test_current_advisor_signature_owner_and_character_faults_publish_no_prefix(self):
        for mutation in (
                lambda f:setattr(f.profile,'bytes',lambda rva,size: b'\0'*size if rva==CURRENT_ADVISOR_SIGNATURES[0][0] else dict(CURRENT_IDEA_SIGNATURES+CURRENT_ADVISOR_SIGNATURES)[rva][:size]),
                lambda f:f.put(f.characters+8,'<Q',f.world),
                lambda f:f.put(f.character+0x68,'<i',2),
                lambda f:f.array(f.characters+0x58,[f.role,f.role])):
            f=self.fixture();mutation(f);value=read_politics(f.reader())
            self.assertEqual(value['advisors']['status'],'unavailable')
            self.assertNotIn('items',value['advisors'])

    def test_current_advisor_ownership_and_membership_are_part_of_the_watch(self):
        for address_name in ('membership','owner','character'):
            f=self.fixture();r=f.reader();value=read_politics(r);r.s.verify()
            self.assertEqual(value['advisors']['status'],'observed')
            address={'membership':f.characters+0x64,'owner':f.characters+8,'character':f.character+0x68}[address_name]
            plan=compile_watch_reads(r.watch);f.put(address,'<i',0)
            self.assertTrue(watch_changed(f,plan))
            with self.assertRaises(LiveReadError):r.s.verify()

    def test_foreign_owner_bad_signature_and_changed_country_fail_closed(self):
        for mutation in (
                lambda f: f.put(f.manager + 0xC8, '<Q', f.world),
                lambda f: setattr(f.profile, 'bytes', lambda rva, size: b'\0' * size),
                lambda f: f.put(f.country + 8, '<i', 2)):
            f = self.fixture()
            mutation(f)
            result = read_politics(f.reader())
            self.assertEqual(result['ideas']['status'], 'unavailable')
            self.assertNotIn('items', result['ideas'])

    def test_empty_membership_is_observed_and_owner_and_membership_are_watched(self):
        f = self.fixture()
        f.array(f.manager + 0x50, [])
        r = f.reader()
        self.assertEqual(read_politics(r)['ideas'], {'status': 'observed', 'items': []})
        r.s.verify()
        self.assertIn((f.manager + 0xC8, 8), r.watch)
        self.assertIn((f.manager + 0x50, 16), r.watch)
        plan = compile_watch_reads(r.watch)
        f.put(f.manager + 0xC8, '<Q', f.world)
        self.assertTrue(watch_changed(f, plan))
        with self.assertRaises(LiveReadError):
            r.s.verify()


if __name__ == '__main__':
    unittest.main()
