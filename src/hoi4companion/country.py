"""Build-pinned, external read-only country observations.

Layout provenance and runtime checks: docs/country-live.md. An unavailable
section never publishes its partially decoded rows. No saves or native calls.
"""
from collections import Counter
import struct

from .live import NativeSample, LiveReadError, WORLD_RVA


class CountryReader:
    def fortifications(self):
        from .fortifications_native import read
        return read(self)

    def doctrines(self):
        from .doctrines_native import read_doctrines
        return read_doctrines(self)

    def metrics(self):
        from .country_metrics_native import read_metrics
        return read_metrics(self)

    def experience(self):
        from .military_native import read_experience
        return read_experience(self)

    def diplomacy(self):
        from .military_native import read_diplomacy
        return read_diplomacy(self)

    def projects(self):
        from .projects_native import read_projects
        return read_projects(self)

    def politics(self):
        from .politics_native import read_politics
        return read_politics(self)

    def events(self):
        from .events_native import read_events
        return read_events(self)

    def decisions(self):
        from .events_native import read_decisions
        return read_decisions(self)

    def __init__(self, process, world, country, tag):
        self.process, self.world, self.country, self.tag = process, world, country, tag
        self.s = NativeSample(process, budget=2 * 1024 * 1024)
        self.u = self.s.number
        self.watch = {}

    def track(self, address, size):
        value = self.s.read(address, size)
        if size:
            self.watch[address, size] = value
        return value

    def pointer(self, address):
        return struct.unpack('<Q', self.track(address, 8))[0]

    def integer(self, address, maximum=2_000_000_000, minimum=0):
        n = struct.unpack('<i', self.track(address, 4))[0]
        if not minimum <= n <= maximum:
            raise LiveReadError('Intero fuori intervallo nel riepilogo paese')
        return n

    def typed(self, address, name):
        if self.process.profile.class_name(self.u(address) - self.process.base) != f'.?AV{name}@@':
            raise LiveReadError(f'Tipo inatteso nel riepilogo: {name}')
        return address

    def array(self, address, maximum=8192, width=8):
        values = self.s.array(address, width, maximum)
        self.track(address, 16)
        if values:
            self.track(self.u(address), len(values) * width)
        if len(set(values)) != len(values):
            raise LiveReadError('Elenco duplicato nel riepilogo paese')
        return values

    def name(self, address):
        header = self.s.read(address, 32)
        length, capacity = struct.unpack_from('<QQ', header, 16)
        if not 0 <= length <= 512 or not max(length, 15) <= capacity <= 65535:
            raise LiveReadError('Nome nativo fuori intervallo')
        pointer = address if capacity == 15 else struct.unpack_from('<Q', header)[0]
        data = self.s.read(pointer, length + 1)
        if data[-1] != 0:
            raise LiveReadError('Nome nativo privo di terminatore')
        # Some pre-existing game templates contain legacy-encoded names.
        try:
            return data[:-1].decode('utf-8')
        except UnicodeDecodeError:
            return data[:-1].decode('cp1252', errors='replace')

    def divisions(self):
        from .country_native import read_section
        return read_section(self,'divisions')

    def manpower(self):
        from .country_native import read_section
        return read_section(self,'manpower')

    def production(self):
        from .country_native import read_section
        return read_section(self,'production')

    def states(self):
        from .country_native import read_section
        return read_section(self,'states')

    def intelligence(self):
        if getattr(self.process.profile,'foreign_interactions_layout',None):
            from .foreign_interactions_native import read_intelligence
            return read_intelligence(self)
        from .country_native import read_section
        return read_section(self,'intelligence')

    def equipment(self):
        from .country_native import read_section
        return read_section(self,'equipment')

    def construction(self):
        from .country_native import read_section
        return read_section(self,'construction')

    def token_name(self, token):
        base = self.process.base
        if self.s.read(base + 0x35E1AB0, 1) != b'\1':
            raise LiveReadError('Cache nomi nativi non inizializzata')
        maximum = self.integer(base + 0x35E1AB4, 1_000_000)
        if not 0 <= token <= maximum:
            raise LiveReadError('Indice nome nativo non valido')
        return self.s.string(self.u(base + 0x35E1AE0) + token * 32)

    def resources(self):
        from .country_native import read_section
        return read_section(self,'resources')


def collect_country(process, world, country, native_tag, country_tag, watch_plan=None):
    result = {'country': country_tag}
    witnesses = {}
    for section in ('divisions', 'manpower', 'states', 'production', 'equipment', 'resources', 'construction', 'intelligence', 'politics', 'events', 'decisions', 'projects', 'experience', 'diplomacy','metrics','doctrines','fortifications'):
        enabled = getattr(process.profile,'country_sections',None)
        if enabled is not None and section not in enabled:
            result[section] = {'status':'unavailable','reason':'Sezione non ancora verificata per questa versione del gioco.'}
            continue
        reader = CountryReader(process, world, country, native_tag)
        try:
            reader.typed(country, 'CCountry')
            if reader.u(process.base + getattr(process.profile,'world_rva',WORLD_RVA)) != world or reader.integer(world + 0x520, 8191) != native_tag:
                raise LiveReadError('Sessione paese cambiata')
            method = getattr(reader, section, None)
            if method is None:
                value = {'status': 'unavailable', 'reason': 'Struttura nativa non ancora qualificata.'}
            else:
                value = method()
            reader.s.verify()
            if section=='fortifications' and watch_plan is not None:
                from .live import compile_watch_reads
                compile_watch_reads({**watch_plan,**reader.watch})
            result[section] = value
            witnesses.update(reader.s.witnesses)
            if watch_plan is not None:
                # Static labels, definitions and RTTI are audited every 5s.
                # Cheap probes watch gameplay values, identity and membership.
                watch_plan.update(reader.watch)
        except (LiveReadError, OSError, ValueError, struct.error, KeyError) as exc:
            result[section] = {'status': 'unavailable', 'reason': str(exc)}
    for (address, size), expected in witnesses.items():
        if process.read(address, size) != expected:
            raise LiveReadError('Riepilogo cambiato durante il campione')
    return result


def localise_country(value, labels):
    """Labels change names, never the native values or identity keys."""
    from copy import deepcopy
    result = deepcopy(value)
    from .tooltips import localise
    for part in ('ideas','advisors'):
        for item in result.get('politics',{}).get(part,{}).get('items',[]):
            item['name']=localise(labels.get(item['id'],labels.get(item.get('character_id'),item['id'])),labels)
            if part=='ideas':item['group_name']=localise(labels.get(item['group'],item['group']),labels)
    for item in result.get('events',{}).get('visible',[]):
        item['name']=localise(labels.get(item['event_id']+'.t',item['event_id']),labels)
        item['options']=[{'id':key,'name':localise(labels.get(key,key),labels)} for key in item.get('option_ids',[])]
    country = result['country']
    for item in result.get('divisions', {}).get('by_type', []):
        item['name'] = labels.get(item['type'], item['type'])
    for item in result.get('states', {}).get('items', []):
        key = f"STATE_{item['id']}"
        item['name'] = labels.get(key, key)
    for section, container in [('production', 'lines'), ('equipment', 'items')]:
        for item in result.get(section, {}).get(container, []):
            key = item.get('equipment', item.get('id'))
            if section == 'production':
                item['equipment_name'] = labels.get(key, key)
            if not item.get('name') or item['name'] == key:
                item['name'] = labels.get(f'{country}_{key}', labels.get(key, key))
    for item in result.get('resources', {}).get('items', []):
        item['name'] = labels.get(item['id'], labels.get('PRODUCTION_MATERIALS_' + item['id'].upper(), item['id']))
    for item in result.get('construction', {}).get('lines', []):
        item['name'] = labels.get(item['building_id'], item['building_id'])
        item['state_name'] = labels.get(f"STATE_{item['state_id']}", str(item['state_id'])) if item['state_id'] else None
    return result
