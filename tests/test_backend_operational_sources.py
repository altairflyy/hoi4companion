"""Static operations preserve identity, source order and unknown conditions."""
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from hoi4companion.backend_core import _request
from hoi4companion._political_import_reference import political_catalog
from hoi4companion._doctrines_reference import doctrine_catalog
from hoi4companion._operation_import_reference import operation_catalog
from hoi4companion._map_topology_reference import load_neighbors
from PIL import Image


class BackendOperationalSourcesTests(unittest.TestCase):
    def test_operations_preserve_geography_equipment_inheritance_variants_and_costs(self):
        with TemporaryDirectory() as folder:
            root=Path(folder)
            sources={
                'common/buildings/all.txt':'buildings={arms_factory={level_cap={state_max=20}} bunker={level_cap={province_max=10}}}',
                'history/states/1.txt':'state={id=1 name=STATE_1 provinces={2 1 3 1} history={victory_points={1 5} 1938.1.1={victory_points={2 10} victory_points={99 1}}}}',
                'common/units/equipment/all.txt':'equipments={tank_equipment={is_archetype=yes interface_category=interface_category_armor} tank_1={archetype=tank_equipment} cycle_a={archetype=cycle_b} cycle_b={archetype=cycle_a}} duplicate_archetypes={assault_tank={archetype=tank_equipment variant_name={tank_role=ROLE_NAME}}}',
                'common/special_projects/projects/all.txt':'one={specialization=land breakthrough_cost={land=1}} unknown={breakthrough_cost={land=variable}} duplicate={breakthrough_cost={land=1 land=2}}',
            }
            for name,source in sources.items():
                path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(source)
            english={'STATE_1':'Province One','VICTORY_POINTS_1':'First City','VICTORY_POINTS_2':'Second City','tank_equipment':'Tank','tank_1':'Model One','HUN_tank_1':'Hungarian Tank','tank_1_short':'Short Tank','ROLE_NAME':'Assault'}
            labels={**english,'tank_1':'Carro','HUN_tank_1':'Carro ungherese'}
            rows,warnings=operation_catalog(root,[],labels,english)
            self.assertEqual(_request(0,'source_operations',game=str(root),mods=[],labels=labels,reference=english),{'rows':rows,'warnings':warnings})

    def test_topology_ignores_absent_land_and_applies_seam_and_impassable_edges(self):
        with TemporaryDirectory() as folder:
            root=Path(folder);(root/'map').mkdir()
            (root/'map/definition.csv').write_text('1;255;0;0;land\n2;0;255;0;land\n3;0;0;255;sea\n4;255;255;255;land\n')
            image=Image.new('RGB',(3,2));image.putdata([(255,0,0),(0,0,255),(0,255,0)]*2);image.save(root/'map/provinces.bmp')
            def check():
                expected={str(k):v for k,v in load_neighbors(root,[]).items()}
                self.assertEqual(_request(0,'source_neighbors',game=str(root),mods=[])['neighbors'],expected)
            check()
            (root/'map/adjacencies.csv').write_text('From;To;Type\n1;2;impassable\n');check()
            (root/'map/definition.csv').write_text('1;255;0;0;land\n2;255;0;0;land\n')
            with self.assertRaisesRegex(ValueError,'Duplicate land province color'):
                _request(0,'source_neighbors',game=str(root),mods=[])

    def test_political_sources_match_roles_broadcasts_and_category_constraints(self):
        with TemporaryDirectory() as folder:
            root=Path(folder)
            sources={
                'common/ideas/laws.txt':'ideas={economy={law=yes allowed={original_tag=HUN} civilian={traits={careful} available={always=no}}} advisors={allowed={tag=HUN} statesman={traits={careful}}}}',
                'common/characters/HUN.txt':'characters={person={name=PERSON advisor={idea_token=minister slot=political_advisor traits={careful}} corps_commander={traits={skilled}} instance={name=OTHER advisor={idea_token=minister slot=political_advisor traits={loyal} available={tag=HUN}} navy_leader={traits={skilled}}}}}',
                'events/all.txt':'country_event={id=notice.1 title=NOTICE trigger={tag=HUN always=no} major=yes option={name=YES trigger={tag=HUN}} option={name=NO}} news_event={id=news.1 title=NEWS trigger={tag=GER}} state_event={id=state.1 title=STATE}',
                'common/decisions/categories/all.txt':'actions={allowed={original_tag=HUN}}',
                'common/decisions/all.txt':'actions={visible={OR={tag=HUN tag=GER}} recruit={available={has_war=yes}}}',
            }
            for name,source in sources.items():
                path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(source)
            english={'careful':'Careful','skilled':'Skilled','loyal':'Loyal','PERSON':'First','OTHER':'Second','NOTICE':'Notice','YES':'Accept'}
            labels={**english,'careful':'Prudente','PERSON':'Primo'}
            rows,warnings=political_catalog(root,[],labels,english)
            result=_request(0,'source_political',game=str(root),mods=[],labels=labels,reference=english)
            self.assertEqual(result,{'rows':rows,'warnings':warnings})
            self.assertEqual(next(r for r in result['rows'] if r['id']=='notice.1')['country_condition'],{'op':'true'})

    def test_doctrines_retain_track_currency_and_only_supported_domains(self):
        with TemporaryDirectory() as folder:
            root=Path(folder)
            for name,source in {'grand_doctrines/base.txt':'mobile={name=MOBILE xp_type=army} sea={name=SEA xp_type=navy} unknown={name=UNKNOWN xp_type=alien}', 'subdoctrines/sub.txt':'fighter={name=FIGHTER xp_type=air track=fighter}', 'ignored.txt':'ignored={name=NO xp_type=army}'}.items():
                path=root/'common/doctrines'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(source)
            labels={'MOBILE':'Mobile warfare'}
            rows,warnings=doctrine_catalog(root,[],labels,labels)
            self.assertEqual(_request(0,'source_doctrines',game=str(root),mods=[],labels=labels,reference=labels),{'rows':rows,'warnings':warnings})
