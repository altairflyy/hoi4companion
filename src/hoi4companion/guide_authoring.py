"""Versioned authoring v1 facade. Rust owns compilation and validation."""
import json
from pathlib import Path

FORMAT='hoi4-guide-authoring'
VERSION=1
_contract=json.loads(Path(__file__).with_name('authoring-actions-v1.json').read_text(encoding='utf8'))
SPECS={key:(value['runtime_kind'],value['category'],' '.join(value['fields']),' '.join(value['required'])) for key,value in _contract['types'].items()}
POLICIES=set(_contract['policies'])
COMMON=set(_contract['common'])
GENERATED=set(_contract['generated'])
IDENTITIES=set(_contract['identities'])
CATEGORY_ALIASES=_contract['category_aliases']
DETAIL_ALIASES=_contract['detail_aliases']
CATEGORIES={'construction','production','politics','characters','army','templates','projects','checkpoints'}

def action_reference():
    from .backend_core import _request
    return _request(0,'authoring_reference')

def compile_document(document):
    from .backend_core import _request
    return _request(0,'authoring_compile',document=document)
