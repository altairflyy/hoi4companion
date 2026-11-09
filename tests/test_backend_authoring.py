"""Authoring compilation preserves explicit policies and ordinary Guide v1."""
import unittest
from hoi4companion.backend_core import _request
from hoi4companion._guide_authoring_reference import compile_document,action_reference

def document(*steps):
    return {'format':'hoi4-guide-authoring','authoring_schema_version':1,'sources':[{'id':'source','type':'manual','text':'Source instructions'}],
            'guide':{'id':'universal','title':'Universal','dependency_policy':'explicit','steps':list(steps)}}

class BackendAuthoringTests(unittest.TestCase):
    def test_every_action_family_compiles_like_existing_authoring_contract(self):
        steps=[{'type':'focus','target':{'id':'F','scope':'tree'}},{'type':'research','target':'Tools UP','upgrade_from':'tools'},
            {'type':'special_project','target':'Flamethrower Tank','depends_on_started':['prior']},
            {'type':'build','building':'Military Factory','state':{'id':64},'quantity':{'mode':'add','value':2}},
            {'type':'build','building':'Land Fort','border':'HUN','quantity':{'mode':'total','value':4}},
            {'type':'production','lines':[{'equipment':'Truck','factories':{'value':1}},{'equipment':'Rifle','factories':{'mode':'remaining'}}]},
            {'type':'law','target':'War Economy','group':'economy'},{'type':'idea','target':'Spirit'},
            {'type':'appoint','trait':'Expert','role':'army_chief'},{'type':'event','event':'Event','option':'Accept'},
            {'type':'decision','target':'Decision','country':'HUN'},{'type':'create_agency'},
            {'type':'agency_upgrade','target':'Upgrade'},{'type':'intelligence_operation','operation':'Network','country':'GER'},
            {'type':'mio','organization':'Concern','equipment':'Rifle'},{'type':'template','template':'Infantry'},
            {'type':'design','name':'Tank','details':{'role':'support'}},{'type':'army','task':'Recruit','details':{'quantity':4}},
            {'type':'diplomacy','task':'Improve Relations','country':'AUS'},
            {'type':'instruction','category':'editorial','instructions':'Follow instructions'},{'type':'reminder','category':'other','instructions':['Note']},
            {'type':'free','lane':'research'}]
        # A declared predecessor is retained, not inferred from proximity.
        steps[1]['key']='prior'
        raw=document(*steps)
        self.assertEqual(_request(0,'authoring_compile',document=raw),compile_document(raw))
        self.assertEqual(_request(0,'authoring_reference'),action_reference())

    def test_invalid_actions_remain_errors_instead_of_becoming_reminders(self):
        for step in [{'type':'research','target':'Tools','unexpected':1},
                     {'type':'production','equipment':'Rifle','factories':{'value':True}},
                     {'type':'appoint','trait':'Expert'},
                     {'type':'build','building':'Factory','state':'Berlin','quantity':{'mode':'add','value':0}},
                     {'type':'instruction','category':'production','instructions':'Act','details':{'verification':{}}}]:
            raw=document(step)
            with self.assertRaises(ValueError) as expected:compile_document(raw)
            with self.assertRaises(ValueError) as actual:_request(0,'authoring_compile',document=raw)
            self.assertEqual(str(actual.exception),str(expected.exception))
