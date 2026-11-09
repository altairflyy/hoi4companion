import copy
import json
from pathlib import Path
import subprocess
import unittest

from hoi4companion.contracts import normalise_state


def sample(items=()):
    return {'usable':True,'sample':{'country':'HUN','session_id':'s',
        'country_overview':{'country':'HUN','doctrines':{'status':'observed','items':list(items)}}}}


class DoctrineContractTests(unittest.TestCase):
    def test_selected_membership_is_qualified_and_empty_membership_is_known(self):
        s=sample([{'id':'grand_battleplan','domain':'land'},{'id':'grand_assault'}])
        f=normalise_state(s)['facts']['doctrines.selected']
        self.assertEqual(f['value'],['grand_assault','grand_battleplan'])
        self.assertEqual(f['status'],'calculated');self.assertEqual(f['source'],'politics_adapter')
        self.assertEqual(normalise_state(sample())['facts']['doctrines.selected']['value'],[])

    def test_partial_foreign_or_malformed_lists_do_not_publish_a_prefix(self):
        states=[]
        for rows in ([{'id':'valid'},{'id':'valid'}],[{'id':'valid'},{}],[{'id':'invalid id'}],['grand_battleplan']):
            states.append(sample(rows))
        s=sample([{'id':'grand_battleplan'}]);s['sample']['country_overview']['doctrines']['status']='partial';states.append(s)
        s=sample([{'id':'grand_battleplan'}]);s['sample']['country_overview']['country']='GER';states.append(s)
        for s in states:self.assertEqual(normalise_state(s)['facts']['doctrines.selected']['status'],'unknown')

    def test_python_and_javascript_adapters_agree(self):
        states=[sample(),sample([{'id':'grand_battleplan'}]),sample([{}]),sample([{'id':'a'},{'id':'a'}])]
        script="const fs=require('fs'),c=require(process.argv[1]);process.stdout.write(JSON.stringify(JSON.parse(fs.readFileSync(0,'utf8')).map(x=>c.state(x).facts['doctrines.selected'])));"
        r=subprocess.run(['node','-e',script,str(Path(__file__).resolve().parents[1]/'src/hoi4companion/web/contracts.js')],input=json.dumps(states),text=True,encoding='utf-8',capture_output=True,check=True)
        self.assertEqual(json.loads(r.stdout),[normalise_state(s)['facts']['doctrines.selected'] for s in states])
