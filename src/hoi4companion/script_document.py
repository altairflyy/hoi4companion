"""Compatibility transport for ordered Jomini nodes; no parsing or evaluation."""
from .clausewitz import Node

def encode(value):
    if not isinstance(value,Node):return value
    return {'entries':[[key,encode(child)] for key,child in value.entries],
            **({'projection':value.projection} if value.projection is not None else {})}

def decode(value):
    if not isinstance(value,dict) or 'entries' not in value:return value
    return Node([(key,decode(child)) for key,child in value['entries']],projection=value.get('projection'))
