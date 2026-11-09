import unittest
from hoi4companion.informal_guides import abbreviation_hint


class InformalGuideTests(unittest.TestCase):
    def test_domains_distinguish_same_abbreviation(self):
        self.assertIn('MIL = military factory',abbreviation_hint('Build 3 MILs and INFRA'))
        self.assertIn('MIL = military factories',abbreviation_hint('Production: assign 3 MILs to Inf'))
        self.assertIsNone(abbreviation_hint('Add MILs'))
        self.assertIsNone(abbreviation_hint('Build MILs and assign MILs to production'))

    def test_hints_are_not_embedded_in_other_words(self):
        self.assertIsNone(abbreviation_hint('Build military infrastructure'))


if __name__ == '__main__': unittest.main()
