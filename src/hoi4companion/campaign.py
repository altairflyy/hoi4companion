"""Qualified campaign identity, separate from native process-session identity."""
import re
from uuid import UUID


PATTERN=r'hoi4:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}:[0-9a-f]{8}'
SOURCE='native_game_unique_id_and_seed'


def valid_id(value):
    return isinstance(value,str) and re.fullmatch(PATTERN,value) is not None and UUID(value.split(':')[1]).int!=0


def observed_id(sample):
    identity=sample.get('campaign_identity',{})
    return identity.get('id') if isinstance(identity,dict) and identity.get('status')=='observed' and identity.get('source')==SOURCE and valid_id(identity.get('id')) else None


def read_campaign(sample,world):
    from types import SimpleNamespace
    from .country_native import read_section
    reader=SimpleNamespace(process=sample.process,s=sample,world=world,country=0,tag=0,watch={})
    return read_section(reader,'campaign')
