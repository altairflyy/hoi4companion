"""Build-pinned, external read-only country observations.

Layout provenance and runtime checks: docs/country-live.md. An unavailable
section never publishes its partially decoded rows. No saves or native calls.
"""
from collections import Counter
import struct

from .live import NativeSample, LiveReadError, WORLD_RVA


class CountryReader:
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
        return data[:-1].decode('utf-8', errors='replace')

    def divisions(self):
        units = self.array(self.country + 0x290)
        types, templates, identities = Counter(), Counter(), set()
        for unit in units:
            self.typed(unit, 'CArmy')
            if self.integer(unit + 0x1D8, 8191) != self.tag:
                raise LiveReadError('Divisione non controllata dalla nazione')
            identity = self.track(unit + 0x18, 8)
            if identity in identities or 0 in struct.unpack('<II', identity):
                raise LiveReadError('Identità divisione assente o duplicata')
            identities.add(identity)
            template = self.typed(self.pointer(unit + 0x3B8), 'CReferencedDivisionTemplate')
            # Expeditionary units legitimately keep their foreign template.
            primary = self.pointer(template + 0x1A8)
            if primary:
                self.typed(primary, 'CSubUnitDefinition')
                kind = self.s.string(primary + 0x18)
            else:
                kind = 'unknown'
            types[kind] += 1
            template_id = self.s.read(template + 8, 8).hex()
            templates[(template_id, self.name(template + 0x20) or self.name(template + 0x40), kind)] += 1
        return {'status': 'observed', 'total': len(units),
                'by_type': [{'type': k, 'name': k, 'count': v} for k, v in sorted(types.items())],
                'by_template': [{'id': k[0], 'name': k[1], 'type': k[2], 'count': v}
                                for k, v in sorted(templates.items())],
                'reason': 'Divisioni controllate, comprese le forze di spedizione. Tipo del battaglione principale del modello.'}

    def manpower(self):
        reserve = self.integer(self.country + 0x334, minimum=-2_000_000_000)
        for state in self.array(self.country + 0x460, 4096):
            self.typed(state, 'CState')
            reserve += self.integer(state + 0x848, minimum=-2_000_000_000)
        if not 0 <= reserve <= 2_000_000_000:
            raise LiveReadError('Manpower disponibile fuori intervallo')
        return {'status': 'partial', 'available': reserve,
                'fielded': self.integer(self.country + 0x348), 'training': None,
                'reason': 'Disponibili e totale schierato letti dal contatore nativo. Coda addestramento non ancora decodificata.'}

    def production(self):
        status = self.typed(self.pointer(self.country + 0xF68), 'CProductionStatus')
        if self.pointer(status + 0x290) != self.country:
            raise LiveReadError('Produzione di un’altra nazione')
        lines = []
        for index, line in enumerate(self.array(status + 0x58, 4096), 1):
            name = self.process.profile.class_name(self.u(line) - self.process.base)
            if name not in ('.?AVCMilitaryProductionLine@@', '.?AVCNavalProductionLine@@'):
                raise LiveReadError('Tipo linea produzione sconosciuto')
            if self.pointer(line + 0x20) != status:
                raise LiveReadError('Linea produzione di un’altra nazione')
            variant = self.typed(self.pointer(line + 0x88), 'CEquipmentVariant')
            equipment = self.typed(self.pointer(variant + 0x3F0), 'CEquipmentType')
            key = self.s.string(equipment + 0x18)
            requested = self.integer(line + 0xE8, 10000)
            unassigned = self.integer(line + 0x40, requested)
            active = self.integer(line + 0x18, 10000)
            damaged = self.integer(line + 0x1C, 10000)
            if active + damaged != requested - unassigned:
                raise LiveReadError('Contatori fabbriche assegnate non concordano')
            cost, daily = struct.unpack('<qq', self.track(line + 0x28, 16))
            if not 0 < cost < 10**15 or not 0 <= daily < 10**15:
                raise LiveReadError('Costo o produzione giornaliera fuori intervallo')
            lines.append({'id': str(index), 'equipment': key,
                          'name': self.name(variant + 0x28) or key,
                          'category': 'naval' if 'Naval' in name else 'military',
                          'requested_factories': requested, 'assigned_factories': requested - unassigned,
                          'active_factories': active, 'damaged_factories': damaged,
                          'output_per_day': daily / cost})
        totals = {}
        for key, off in [('military', 0x2B8), ('dockyards', 0x318), ('civilian', 0x378)]:
            raw = struct.unpack('<q', self.track(status + off, 8))[0]
            if not 0 <= raw <= 10**12:
                raise LiveReadError('Contatore fabbriche fuori intervallo')
            totals[key] = raw // 100000
        return {'status': 'observed', 'lines': lines, 'effective_factories': totals,
                'reason': 'Fabbriche effettivamente assegnate; richieste conservate separatamente. Uscita teorica corrente in unità/giorno.'}

    def states(self):
        registry = self.u(self.process.base + 0x332EE28)
        definitions = {}
        for definition in self.array(registry + 0x40, 512):
            if self.process.profile.class_name(self.u(definition) - self.process.base) != '.?AVCBuildingTemplate@@':
                continue  # The registry has a typed null sentinel.
            key = self.s.string(definition + 0x270)
            if key in ('infrastructure', 'industrial_complex', 'arms_factory', 'dockyard'):
                if key in definitions:
                    raise LiveReadError('Definizione edificio duplicata')
                definitions[key] = self.integer(definition + 0x2E0, 511)
        if len(definitions) != 4:
            raise LiveReadError('Definizioni edifici mancanti')
        items, identifiers = [], set()
        for state in self.array(self.world + 0x2C8, 4096):
            if not state:
                continue
            self.typed(state, 'CState')
            owner, controller = self.integer(state + 0xC8, 8191), self.integer(state + 0xCC, 8191)
            if self.tag not in (owner, controller):
                continue
            identifier = self.integer(state + 0x58, 100000, 1)
            if identifier in identifiers:
                raise LiveReadError('Identità territorio duplicata')
            identifiers.add(identifier)
            mapping = self.s.array(state + 0x140, 4, 512)
            self.track(state + 0x140, 16)
            if mapping:
                self.track(self.u(state + 0x140), len(mapping) * 4)
            buildings = self.array(state + 0x158, 512)
            levels = {}
            for key, slot in definitions.items():
                if slot >= len(mapping):
                    raise LiveReadError('Mappa edifici incompleta')
                index = mapping[slot]
                if index == -1:
                    levels[key] = 0
                    continue
                if not 0 <= index < len(buildings):
                    raise LiveReadError('Indice edificio non valido')
                building = self.typed(buildings[index], 'CBuilding')
                level = struct.unpack('<h', self.track(building + 0x40, 2))[0]
                if not 0 <= level <= 1000:
                    raise LiveReadError('Livello edificio non valido')
                levels[key] = level
            items.append({'id': identifier, 'name': f'STATE_{identifier}',
                          'owned': owner == self.tag, 'controlled': controller == self.tag,
                          'civilian_factories': levels['industrial_complex'],
                          'military_factories': levels['arms_factory'],
                          'dockyards': levels['dockyard'], 'infrastructure': levels['infrastructure']})
        return {'status': 'observed', 'items': sorted(items, key=lambda x: x['id']),
                'reason': 'Edifici costruiti nei territori posseduti o controllati. Danni, occupazione e bonus modificano le fabbriche effettivamente disponibili.'}

    def equipment(self):
        status = self.typed(self.pointer(self.country + 0xF68), 'CProductionStatus')
        if self.pointer(status + 0x290) != self.country:
            raise LiveReadError('Scorte di un’altra nazione')
        pool = self.typed(status + 0x200, 'CEquipmentVariantPool')
        data, capacity, count = struct.unpack('<Qii', self.track(pool + 0x20, 16))
        if not 0 <= count <= capacity <= 4096 or (count and not data):
            raise LiveReadError('Elenco scorte non valido')
        raw = self.track(data, count * 16) if count else b''
        amounts, variants, seen = Counter(), [], set()
        for offset in range(0, len(raw), 16):
            variant, amount = struct.unpack_from('<Qq', raw, offset)
            if variant in seen or not 0 <= amount <= 10**16:
                raise LiveReadError('Variante scorte duplicata o quantità non valida')
            seen.add(variant)
            self.typed(variant, 'CEquipmentVariant')
            equipment = self.typed(self.pointer(variant + 0x3F0), 'CEquipmentType')
            key = self.s.string(equipment + 0x18)
            amounts[key] += amount
            variants.append({'equipment': key, 'name': self.name(variant + 0x28) or key,
                             'stockpile': amount / 100000})
        return {'status': 'partial',
                'items': [{'id': key, 'name': key, 'stockpile': amount / 100000,
                           'balance_per_day': None, 'needed': None} for key, amount in sorted(amounts.items())],
                'variants': variants,
                'reason': 'Quantità lorde nel magazzino nativo, incluse le varianti estere. Il saldo della Logistica sottrae anche fabbisogni: fabbisogno e saldo netto giornaliero non ancora qualificati.'}

    def token_name(self, token):
        base = self.process.base
        if self.s.read(base + 0x35E1AB0, 1) != b'\1':
            raise LiveReadError('Cache nomi nativi non inizializzata')
        maximum = self.integer(base + 0x35E1AB4, 1_000_000)
        if not 0 <= token <= maximum:
            raise LiveReadError('Indice nome nativo non valido')
        return self.s.string(self.u(base + 0x35E1AE0) + token * 32)

    def resources(self):
        manager = self.typed(self.pointer(self.country + 0x11F8), 'CCountryResources')
        if self.integer(manager + 0x20, 8191) != self.tag:
            raise LiveReadError('Risorse di un’altra nazione')
        # The same initialized token table gives serializer field identities.
        expected = {0x2FAC: 'produced', 0x30C2: 'imported', 0x30C3: 'to_use',
                    0x30C5: 'exported'}
        if any(self.token_name(token) != name for token, name in expected.items()):
            raise LiveReadError('Identità campi risorse non confermata')
        registry = self.u(self.process.base + 0x332F088)
        definitions = self.array(registry + 0x28, 128)
        resources = {}
        for index, definition in enumerate(definitions[1:], 1):
            self.typed(definition, 'CStrategicResource')
            token = self.integer(definition + 8, 1_000_000)
            if self.integer(definition + 0xD8, 127) != index:
                raise LiveReadError('Indice risorsa non coerente')
            key = self.token_name(token)
            if key in resources:
                raise LiveReadError('Risorsa duplicata')
            resources[key] = token
        groups = {}
        # CAED60 sums the second to_use collection (+2E8) and +398.
        for name, offset in [('produced', 0x28), ('imported', 0x188), ('available', 0x2E8),
                             ('demand', 0x398), ('exported', 0x5B0)]:
            data, capacity, count = struct.unpack('<Qii', self.track(manager + offset + 8, 16))
            if count != len(resources) + 1 or not count <= capacity <= 128:
                raise LiveReadError('Elenco risorse incompleto')
            raw = self.track(data, count * 16)
            values = {}
            for n in range(count):
                amount, token = struct.unpack_from('<qI', raw, n * 16)
                if abs(amount) > 10**15 or token in values:
                    raise LiveReadError('Quantità o identità risorsa non valida')
                values[token] = amount / 100000
            if not set(resources.values()) <= values.keys():
                raise LiveReadError('Risorsa mancante nel contenitore')
            groups[name] = values
        return {'status': 'observed', 'items': [
            {'id': key, 'name': key, 'available': groups['available'][token],
             'used': -groups['demand'][token] or 0,
             'balance': groups['available'][token] + groups['demand'][token],
             'produced': groups['produced'][token], 'imported': groups['imported'][token],
             'exported': groups['exported'][token]}
            for key, token in resources.items()],
            'reason': 'Risorse industriali: disponibilità e domanda corrente; saldo confrontato con il pannello Produzione. Petrolio distinto dalle scorte di carburante.'}


def collect_country(process, world, country, native_tag, country_tag, watch_plan=None):
    result = {'country': country_tag}
    witnesses = {}
    for section in ('divisions', 'manpower', 'states', 'production', 'equipment', 'resources'):
        reader = CountryReader(process, world, country, native_tag)
        try:
            reader.typed(country, 'CCountry')
            if reader.u(process.base + WORLD_RVA) != world or reader.integer(world + 0x520, 8191) != native_tag:
                raise LiveReadError('Sessione paese cambiata')
            method = getattr(reader, section, None)
            if method is None:
                value = {'status': 'unavailable', 'reason': 'Struttura nativa non ancora qualificata.'}
            else:
                value = method()
            reader.s.verify()
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
    country = result['country']
    for item in result.get('divisions', {}).get('by_type', []):
        item['name'] = labels.get(item['type'], item['type'])
    for item in result.get('states', {}).get('items', []):
        key = f"STATE_{item['id']}"
        item['name'] = labels.get(key, key)
    for section, container in [('production', 'lines'), ('equipment', 'items')]:
        for item in result.get(section, {}).get(container, []):
            key = item.get('equipment', item.get('id'))
            if not item.get('name') or item['name'] == key:
                item['name'] = labels.get(f'{country}_{key}', labels.get(key, key))
    for item in result.get('resources', {}).get('items', []):
        item['name'] = labels.get(item['id'], labels.get('PRODUCTION_MATERIALS_' + item['id'].upper(), item['id']))
    return result
