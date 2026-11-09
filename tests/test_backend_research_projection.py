"""The API definition and HUD decision must expose the same campaign target."""
import unittest
from hoi4companion.backend_core import _request
from hoi4companion._relative_research_reference import project
from test_relative_research import fixture,state

class BackendResearchProjectionTests(unittest.TestCase):
    def test_persisted_bindings_and_unbound_placeholders_match_public_projection(self):
        guide=fixture()
        for bindings in ({},{'r0':'b','r1':'c'},{'r0':'missing'},None):
            self.assertEqual(_request(0,'guide_research_projection',guide=guide,bindings=bindings),project(guide,bindings))

    def test_app_definition_uses_qualified_binding_and_discards_it_in_offline_projection(self):
        guide=fixture();handle=_request(0,'app_open',guides=[guide]);self.addCleanup(lambda:_request(0,'app_release',handle=handle))
        _request(0,'app_progress',handle=handle,guide_id=guide['id'])
        _request(0,'app_state',handle=handle,value=state(['a']))
        packet=_request(0,'app_projection',handle=handle,guide_id=guide['id'],live=True)
        row=packet['guide']['tracks']['research'][0]
        self.assertEqual(row['target'],'b')
        self.assertEqual(row['action']['entity']['id'],'b')
        self.assertEqual(packet['definition_revision'],packet['guide']['runtime_definition_revision'])
        self.assertEqual(_request(0,'app_guide',handle=handle,guide_id=guide['id'])['tracks']['research'][0]['target'],'b')
        offline=_request(0,'app_projection',handle=handle,guide_id=guide['id'],live=False)
        self.assertEqual(offline['guide']['tracks']['research'][0]['target'],'unbound-up:r0')
