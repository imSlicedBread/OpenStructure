"""Independent IFC4 syntax/schema/geometry gate. No OpenStructure parser is used.

Install tools/ifc-validation-requirements.txt in a development environment.
Run: python tools/validate-ifc.py fixtures/wall-exchange.ifc
"""
import json
import math
import sys
import uuid
import zipfile

import ifcopenshell
import ifcopenshell.geom
import ifcopenshell.util.placement
import ifcopenshell.util.unit
import ifcopenshell.validate


def validate(path, native_path=None):
    model = ifcopenshell.open(path)
    assert model.schema == "IFC4", model.schema
    logger = ifcopenshell.validate.json_logger()
    ifcopenshell.validate.validate(model, logger, express_rules=True)
    assert not logger.statements, json.dumps(logger.statements, default=str, indent=2)
    assert ifcopenshell.util.unit.calculate_unit_scale(model) == 1
    assert len(model.by_type("IfcProject")) == 1
    assert len(model.by_type("IfcSite")) == 1
    assert len(model.by_type("IfcBuilding")) == 1
    if native_path:
        with zipfile.ZipFile(native_path) as archive:
            native = json.loads(archive.read("model.json"))
        # Compare standard IFC GUID expansion against the actual source UUIDs,
        # independently of the Rust reader/writer's shared GUID implementation.
        for kind, entities in [("IfcProject", [native["project"]]),
                               ("IfcSite", native["sites"].values()),
                               ("IfcBuilding", native["buildings"].values()),
                               ("IfcBuildingStorey", native["levels"].values()),
                               ("IfcWall", native["walls"].values())]:
            assert {str(uuid.UUID(ifcopenshell.guid.expand(e.GlobalId))) for e in model.by_type(kind)} == {e["header"]["id"] for e in entities}
        for wall in model.by_type("IfcWall"):
            ident = str(uuid.UUID(ifcopenshell.guid.expand(wall.GlobalId)))
            source = native["walls"][ident]["parameters"]
            matrix = ifcopenshell.util.placement.get_local_placement(wall.ObjectPlacement)
            dx, dy = source["end"]["x"]-source["start"]["x"], source["end"]["y"]-source["start"]["y"]
            length = math.hypot(dx, dy)
            solid = wall.Representation.Representations[0].Items[0]
            expected = [source["start"]["x"], source["start"]["y"], dx/length, dy/length,
                        length, source["thickness"], source["height"]]
            actual = [matrix[0, 3], matrix[1, 3], matrix[0, 0], matrix[1, 0],
                      solid.SweptArea.XDim, solid.SweptArea.YDim, solid.Depth]
            assert all(math.isclose(a, b, abs_tol=1e-8) for a, b in zip(actual, expected))
    settings = ifcopenshell.geom.settings()
    settings.set(settings.USE_WORLD_COORDS, True)
    walls = []
    for wall in model.by_type("IfcWall"):
        assert len(wall.ContainedInStructure) == 1
        storey = wall.ContainedInStructure[0].RelatingStructure
        assert storey.is_a("IfcBuildingStorey")
        assert len(storey.Decomposes) == 1
        assert storey.Decomposes[0].RelatingObject.is_a("IfcBuilding")
        matrix = ifcopenshell.util.placement.get_local_placement(wall.ObjectPlacement)
        assert math.isclose(float(matrix[2, 3]), storey.Elevation, abs_tol=1e-8)
        # Open CASCADE, independently from the Rust prism kernel and reader.
        shape = ifcopenshell.geom.create_shape(settings, wall)
        xyz = shape.geometry.verts
        vertices = [xyz[i:i+3] for i in range(0, len(xyz), 3)]
        indices = shape.geometry.faces
        volume = 0.0
        for i in range(0, len(indices), 3):
            a, b, c = (vertices[indices[i+j]] for j in range(3))
            volume += (a[0]*(b[1]*c[2]-b[2]*c[1]) +
                       a[1]*(b[2]*c[0]-b[0]*c[2]) +
                       a[2]*(b[0]*c[1]-b[1]*c[0])) / 6
        solid = wall.Representation.Representations[0].Items[0]
        profile = solid.SweptArea
        expected = profile.XDim * profile.YDim * solid.Depth
        assert volume > 0 and math.isclose(volume, expected, rel_tol=1e-7)
        low = [min(v[i] for v in vertices) for i in range(3)]
        high = [max(v[i] for v in vertices) for i in range(3)]
        assert math.isclose(low[2], storey.Elevation, abs_tol=1e-8)
        assert math.isclose(high[2]-low[2], solid.Depth, abs_tol=1e-8)
        walls.append(dict(global_id=wall.GlobalId, volume_m3=volume,
                          bounds_metres=[low, high], triangles=len(indices)//3))
    assert walls, "Fixture must exercise wall geometry"
    # Frozen foundation smoke fixture: length 7, thickness .3, height 3.5,
    # upper level at 3 m. Catch a shared-but-wrong interpretation of dimensions.
    if path.replace("\\", "/").endswith("/wall-exchange.ifc"):
        assert len(walls) == 1
        assert math.isclose(walls[0]["volume_m3"], 7.35, abs_tol=1e-8)
        for actual, expected in zip(sum(walls[0]["bounds_metres"], []), [0, -.15, 3, 7, .15, 6.5]):
            assert math.isclose(actual, expected, abs_tol=1e-8)
    if path.replace("\\", "/").endswith("/rotated-walls.ifc"):
        assert len(walls) == 2
        ordered = sorted(walls, key=lambda w: w["bounds_metres"][0][2])
        expected_bounds = [[-2.09, .88, 0, 2.09, 4.12, 3.5],
                           [4.88, -7.09, 4.2, 8.12, -2.91, 7.7]]
        for wall, expected in zip(ordered, expected_bounds):
            assert math.isclose(wall["volume_m3"], 5.25, abs_tol=1e-8)
            for actual, value in zip(sum(wall["bounds_metres"], []), expected):
                assert math.isclose(actual, value, abs_tol=1e-8)
    print(json.dumps(dict(validator=ifcopenshell.version, schema=model.schema,
                         schema_errors=0, walls=walls), indent=2))


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3):
        raise SystemExit("Usage: python tools/validate-ifc.py <fixture.ifc> [source.osb]")
    validate(sys.argv[1], sys.argv[2] if len(sys.argv) == 3 else None)
