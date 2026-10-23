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
    """Use the caller's witnessed reads; failure does not invalidate other domains.

    Build-bound evidence: serializer 1F2F19/1F2F2D and parser 1E8C6E.
    Both serialize/restore game_unique_seed +720 and game_unique_id +728.
    """
    from .live import LiveReadError
    try:
        profile=sample.process.profile
        if profile.bytes(0x1F2F2D,18).hex()!='4c8d86280700004533c9ba803b0000488bcb':
            raise LiveReadError('Firma identità campagna non riconosciuta')
        # Read-only validation of the initialized serialization token.
        if sample.number(sample.process.base+0x3423158,'<I')!=0x3B80:
            raise LiveReadError('Token identità campagna non qualificato')
        seed=sample.number(world+0x720,'<I');raw=sample.string(world+0x728)
        identity=f'hoi4:{raw.lower()}:{seed:08x}'
        if len(raw)!=36 or not valid_id(identity):raise LiveReadError('UUID campagna non valido')
        return {'status':'observed','id':identity,'source':SOURCE}
    except (LiveReadError,ValueError,AttributeError) as exc:
        return {'status':'unknown','id':None,'source':'','reason':str(exc)}
