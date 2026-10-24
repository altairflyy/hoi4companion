"""Build-pinned, external read-only country observations.

Layout provenance and runtime checks: docs/country-live.md. An unavailable
section never publishes its partially decoded rows. No saves or native calls.
"""
from collections import Counter
import struct

from .live import NativeSample, LiveReadError, WORLD_RVA


class CountryReader:
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

    def intelligence(self):
        """Creation flags, not agency upgrades or percentage estimates.

        Exact-build getter 6CF7F0, serializer FDF3A0, start FDE7C0
        and cancel FDAA50 corroborate C0=is_created and C1=in_creation.
        """
        agency = self.typed(self.pointer(self.country + 0xFC0), 'CIntelligenceAgency')
        if self.pointer(agency + 0x78) != self.country:
            raise LiveReadError('Agenzia di intelligence di un’altra nazione')
        created, creating = self.track(agency + 0xC0, 2)
        if created not in (0, 1) or creating not in (0, 1):
            raise LiveReadError('Indicatori creazione agenzia non validi')
        return {'status': 'observed', 'is_created': bool(created),
                'in_creation': bool(creating),
                'reason': 'Creazione avviata e agenzia operativa sono indicatori nativi distinti. Migliorie, operazioni e percentuali non determinate.'}

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

    def construction(self):
        """Own queue membership, not a completion history or an ETA.

        Exact-build serializers: E71CB0, F6E660, 14B59B0, 10DB880;
        geographic joins must reciprocally contain the selected building.
        """
        production = self.typed(self.pointer(self.country + 0xF68), 'CProductionStatus')
        if self.pointer(production + 0x290) != self.country:
            raise LiveReadError('Costruzioni di un’altra nazione')
        entries = self.array(production + 0x70, 2048)
        # Empty is observed only after validating the owner and full container.
        if not entries:
            return {'status': 'observed', 'lines': [], 'reason': 'Coda di costruzione vuota osservata.'}
        registry = self.typed(self.u(self.process.base + 0x332EE28), 'CBuildingDatabase')
        definitions = self.array(registry + 0x40, 512)
        def location_member(header, identifier, maximum):
            data, capacity, count = struct.unpack('<Qii', self.track(header, 16))
            if not 0 < identifier < count <= capacity <= maximum or not data:
                raise LiveReadError('Ubicazione costruzione fuori dal catalogo')
            return self.pointer(data + identifier * 8)
        lines, handles, incomplete = [], set(), False
        for position, line in enumerate(entries, 1):
            kind = self.process.profile.class_name(self.u(line) - self.process.base)
            if kind != '.?AVCBuildingProductionLine@@':
                incomplete = True  # Railways/other line layouts remain unqualified.
                continue
            if self.pointer(line + 0x20) != production:
                raise LiveReadError('Ordine di costruzione di un’altra nazione')
            handle = self.track(line + 8, 8)
            if handle in handles or struct.unpack('<II', handle)[0] != 0x3B or not struct.unpack('<II', handle)[1]:
                raise LiveReadError('Identità ordine di costruzione assente o duplicata')
            handles.add(handle)
            building = self.typed(self.pointer(line + 0x70), 'CBuilding')
            status = self.typed(self.pointer(building + 0x1D8), 'CBuildingStatus')
            template = self.typed(self.pointer(building + 0x1E0), 'CBuildingTemplate')
            ordinal = self.u(template + 0x2E0, '<i')
            if not 0 <= ordinal < len(definitions) or definitions[ordinal] != template:
                raise LiveReadError('Definizione costruzione fuori dal catalogo corrente')
            key = self.s.string(template + 0x270)
            level = struct.unpack('<h', self.track(building + 0x40, 2))[0]
            if not 0 <= level <= 1000:
                raise LiveReadError('Livello costruito non valido')
            amount = self.integer(line + 0x48, 1000)
            repairs = self.integer(line + 0x78, 1000)
            conversion = self.track(line + 0x8C, 1)[0]
            if conversion not in (0, 1):
                raise LiveReadError('Indicatore conversione non valido')
            row = {'id': handle.hex(), 'position': position, 'building_id': key,
                   'amount': amount, 'built_level': level,
                   'active_factories': self.integer(line + 0x18, 10000),
                   'repair_levels': repairs, 'conversion': bool(conversion),
                   'state_id': None, 'province_id': None, 'location_status': 'unavailable'}
            flags = self.integer(status + 0x68, 0x7FFFFFFF)
            identifier = self.integer(status + 0x6C, 100000, 1)
            national_override = self.integer(status + 0x70, 8191)
            if flags & 1:
                province = self.typed(location_member(self.world + 0x2B0, identifier, 20000), 'CProvince')
                if self.integer(province + 0xA4, 100000, 1) != identifier or province + 0x190 != status:
                    raise LiveReadError('Identità provincia costruzione non coerente')
                state = self.typed(self.pointer(province + 0xC0), 'CState')
                row['province_id'] = identifier
                own_province = self.integer(province + 0x188, 8191) == self.tag
            else:
                state = self.typed(location_member(self.world + 0x2C8, identifier, 4096), 'CState')
                if state + 0x120 != status:
                    raise LiveReadError('Contenitore regione costruzione non coerente')
                own_province = True
            state_id = self.integer(state + 0x58, 100000, 1)
            if location_member(self.world + 0x2C8, state_id, 4096) != state:
                raise LiveReadError('Identità regione costruzione non coerente')
            owner = self.integer(state + 0xC8, 8191)
            controller = self.integer(state + 0xCC, 8191)
            if national_override == 0 and own_province and owner == controller == self.tag:
                mapping = self.s.array(status + 0x20, 4, 512)
                self.track(status + 0x20, 16)
                if mapping:
                    self.track(self.u(status + 0x20), len(mapping) * 4)
                members = self.array(status + 0x38, 512)
                if ordinal >= len(mapping) or not 0 <= mapping[ordinal] < len(members) or members[mapping[ordinal]] != building:
                    raise LiveReadError('Edificio non appartenente alla regione/provincia dichiarata')
                row.update(state_id=state_id, location_status='observed')
            else:
                incomplete = True
            lines.append(row)
        return {'status': 'partial' if incomplete else 'observed', 'lines': lines,
                'reason': 'Ordini in coda, compresi quelli senza fabbriche attive. Riparazioni e conversioni distinte. La scomparsa di un ordine non prova il completamento.'}

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
    for section in ('divisions', 'manpower', 'states', 'production', 'equipment', 'resources', 'construction', 'intelligence', 'politics', 'events', 'decisions', 'projects'):
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
