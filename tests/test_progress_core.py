"""Native campaign transitions compared with the preserved, independent oracle."""
import itertools
import unittest
from hoi4companion.progress_core import NativeRuns
from hoi4companion._guide_progress_reference import RunStore as Reference


class ProgressCoreTests(unittest.TestCase):
    def test_ordered_transitions_and_rewinds_match_reference(self):
        reference=Reference(id_factory=lambda sequence=itertools.count(): 'run-'+str(next(sequence)))
        native=NativeRuns(id_factory=lambda sequence=itertools.count(): 'run-'+str(next(sequence)))
        first='hoi4:11111111-1111-4111-8111-111111111111:00000001'
        second='hoi4:22222222-2222-4222-8222-222222222222:00000002'
        operations=[('ensure',('one',{'done':True,'no':False}),{}),('ensure',('two',),{}),
            ('bind',('session',),{}),('update',('one',{'done':False}),{'started':{'build':True}}),
            ('bind_campaign',(first,'HUN','session',100),{}),('bind_campaign',(first,'HUN','session',101),{}),
            ('create',('one',),{}),('bind_campaign',(first,'HUN','new-session',102),{}),
            ('update',('one',{'done':True}),{}),('bind_campaign',(first,'HUN','new-session',50),{}),
            ('bind_campaign',(second,'POL','new-session',50),{}),
            ('bind_campaign',(first,'HUN','new-session',51),{})]
        try:
            for method,args,kwargs in operations:
                with self.subTest(method=method,args=args):
                    self.assertEqual(getattr(native,method)(*args,**kwargs),getattr(reference,method)(*args,**kwargs))
                    self.assertEqual(native.snapshot(),reference.snapshot())
                    self.assertEqual(native.metadata(),reference.metadata())
                    self.assertEqual(native.history('one'),reference.history('one'))
        finally:native.close()

    def test_errors_do_not_mutate_and_native_replies_are_isolated(self):
        with NativeRuns(id_factory=lambda:'same') as native:
            original=native.ensure('g');original['checks']['other']=True
            self.assertEqual(native.get('g')['checks'],{})
            snapshot=native.snapshot()
            with self.assertRaises(ValueError):native.create('g')
            self.assertEqual(native.snapshot(),snapshot)
            for campaign in ('not-a-campaign','hoi4:00000000-0000-0000-0000-000000000000:00000001'):
                with self.assertRaises(ValueError):native.bind_campaign(campaign,'HUN','s',1)
                self.assertEqual(native.snapshot(),snapshot)
