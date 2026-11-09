from unittest import TestCase
from copy import deepcopy
from hoi4companion.backend_core import _request
from hoi4companion._production_milestones_reference import annotate

class NativeMilestoneAnnotationTests(TestCase):
    def test_source_action_column_anchors_only_and_declared_dependencies_win(self):
        text='[Table row]\n[Column 1] Appoint expert\n[Column 2] Radio\n[Column 3] Industry\n[Table row]\n[Column 1] Produce rifles\n[Column 2] Tools\n[Column 3] Army'
        guide={'dependency_policy':'explicit','sources':[{'id':'source','type':'file','text':text}],'tracks':{'operations':[
            {'key':'expert','action':{'kind':'manual','params':{'role':'high_command','instructions':['Appoint expert']}},'completion':{'op':'fact','key':'politics.advisors','test':'contains','value':'expert'},'evidence':[{'source_id':'source','quote':'Appoint expert'}]},
            {'key':'production','action':{'kind':'manual','params':{'category':'production','instructions':['Produce rifles']}},'evidence':[{'source_id':'source','quote':'Produce rifles'}]}]}}
        expected=deepcopy(guide);changes=annotate(expected)
        self.assertEqual(_request(0,'import_annotate_milestones',guide=guide),{'guide':expected,'changes':changes})
        self.assertEqual(_request(0,'import_annotate_milestones',guide=expected),{'guide':expected,'changes':[]})
        guide['tracks']['operations'][1]['depends_on']=[]
        self.assertEqual(_request(0,'import_annotate_milestones',guide=guide),{'guide':guide,'changes':[]})
