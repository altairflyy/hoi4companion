"""Build-pinned, read-only relation and operative mission observations.

No commands or timers: changed bytes join the existing native watch plan.
"""
import struct
from .live import LiveReadError
from .foreign_interactions import MISSIONS, STATES

MISSION_CLASSES=('CNoMission','CBuildIntelNetwork','CQuietIntelNetwork','CCounterIntelligence',
                 'CRootOutResistance','CBoostIdeology','CControlTrade','CDiplomaticPressure','CPropaganda')


def read_relations(reader,status):
    from .country_native import read_section
    return read_section(reader,'foreign_relations',diplomacy_status=status)


def read_intelligence(reader):
    from .country_native import read_section
    return read_section(reader,'intelligence')
