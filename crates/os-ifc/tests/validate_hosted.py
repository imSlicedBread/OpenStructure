"""Independent IFC4 hosted-opening gate; invoked by the ignored Rust integration test.

Uses the existing development-only IfcOpenShell environment. Unlike the baseline
tools/validate-ifc.py (which expects gross wall volume), check actual OCC voids,
placement chains, source UUIDs, source dimensions and physical door hinge/swing.
No OpenStructure IFC reader or native geometry implementation is used here.
"""
import csv
import json
import math
from pathlib import Path
import sys
import uuid

import ifcopenshell
import ifcopenshell.geom
import ifcopenshell.util.placement
import ifcopenshell.util.shape
import ifcopenshell.validate
import numpy as np


def native_id(entity):
    return str(uuid.UUID(ifcopenshell.guid.expand(entity.GlobalId)))


def matrix(entity):
    return ifcopenshell.util.placement.get_local_placement(entity.ObjectPlacement)


def validate(path):
    model = ifcopenshell.open(str(path))
    assert model.schema == "IFC4"
    logger = ifcopenshell.validate.json_logger()
    ifcopenshell.validate.validate(model, logger, express_rules=True)
    assert not logger.statements, json.dumps(logger.statements, default=str, indent=2)
    with path.with_suffix(".tsv").open(encoding="utf-8", newline="") as stream:
        expected = {r[0]: r for r in csv.reader(stream, delimiter="\t")}
    fillings = model.by_type("IfcDoor") + model.by_type("IfcWindow")
    assert {native_id(e) for e in fillings} == set(expected)
    assert len(model.by_type("IfcOpeningElement")) == len(fillings) == 4
    assert len(model.by_type("IfcRelVoidsElement")) == 4
    assert len(model.by_type("IfcRelFillsElement")) == 4
    assert len(model.by_type("IfcDoorType") + model.by_type("IfcWindowType")) == len(
        {r[10] for r in expected.values() if r[10]}
    )
    settings = ifcopenshell.geom.settings()
    settings.set(settings.USE_WORLD_COORDS, True)
    cut_volume = {}
    for fill in fillings:
        r = expected[native_id(fill)]
        offset, sill, width, height = map(float, r[4:8])
        sx, sy, ex, ey, elevation = map(float, r[12:17])
        direction = np.array([ex - sx, ey - sy, 0.]) / math.hypot(ex - sx, ey - sy)
        normal = np.array([-direction[1], direction[0], 0.])
        assert fill.Name == r[1] and fill.is_a() == "Ifc" + r[3]
        assert fill.Representation is None  # omission is explicit, never a fake body
        assert math.isclose(fill.OverallWidth, width) and math.isclose(fill.OverallHeight, height)
        assert len(fill.FillsVoids) == len(fill.ContainedInStructure) == 1
        void = fill.FillsVoids[0].RelatingOpeningElement
        assert len(void.VoidsElements) == 1 and not void.ContainedInStructure
        host = void.VoidsElements[0].RelatingBuildingElement
        assert native_id(host) == r[2]
        assert fill.ContainedInStructure[0].RelatingStructure == host.ContainedInStructure[0].RelatingStructure
        assert void.ObjectPlacement.PlacementRelTo == host.ObjectPlacement
        assert fill.ObjectPlacement.PlacementRelTo == void.ObjectPlacement
        expected_origin = np.array([sx, sy, elevation + sill]) + offset * direction
        np.testing.assert_allclose(matrix(void)[:3, 3], expected_origin, atol=1e-8)
        np.testing.assert_allclose(matrix(host)[:3, 0], direction, atol=1e-8)
        shape = ifcopenshell.geom.create_shape(settings, void)
        assert math.isclose(ifcopenshell.util.shape.get_volume(shape.geometry), width * height * .3, abs_tol=1e-8)
        # Transform independently tessellated world vertices back into the host
        # frame: exact first jamb, sill, width, height, and full centered depth.
        points = ifcopenshell.util.shape.get_vertices(shape.geometry)
        host_points = (np.linalg.inv(matrix(host)) @ np.column_stack((points, np.ones(len(points)))).T).T[:, :3]
        np.testing.assert_allclose(host_points.min(axis=0), [offset, -.15, sill], atol=1e-8)
        np.testing.assert_allclose(host_points.max(axis=0), [offset + width, .15, sill + height], atol=1e-8)
        if r[10]:
            assert len(fill.IsTypedBy) == 1
            ty = fill.IsTypedBy[0].RelatingType
            assert native_id(ty) == r[10] and ty.Name == r[11]
            assert len(ty.Types) == 1 and len(ty.Types[0].RelatedObjects) == 4
            assert fill.PredefinedType is None
        else:
            assert not fill.IsTypedBy
            ty = fill
        if r[3] == "Door":
            # buildingSMART: hinge at local x=0 for SINGLE_SWING_LEFT,
            # local x=width for RIGHT; swing always toward filling local +Y.
            operation = ty.OperationType
            assert operation in ("SINGLE_SWING_LEFT", "SINGLE_SWING_RIGHT")
            local_hinge = 0. if operation == "SINGLE_SWING_LEFT" else width
            actual_hinge = (matrix(fill) @ [local_hinge, 0., 0., 1.])[:3]
            expected_hinge = expected_origin + (0. if r[8] == "Start" else width) * direction
            np.testing.assert_allclose(actual_hinge, expected_hinge, atol=1e-8)
            np.testing.assert_allclose(matrix(fill)[:3, 1], normal * (1. if r[9] == "Left" else -1.), atol=1e-8)
        else:
            assert ty.PartitioningType == "SINGLE_PANEL"
        cut_volume[host.id()] = cut_volume.get(host.id(), 0.) + width * height * .3
    for wall in model.by_type("IfcWall"):
        solid = wall.Representation.Representations[0].Items[0]
        assert [solid.SweptArea.XDim, solid.SweptArea.YDim, solid.Depth] == [5., .3, 3.5]
        shape = ifcopenshell.geom.create_shape(settings, wall)
        volume = ifcopenshell.util.shape.get_volume(shape.geometry)
        assert math.isclose(volume, 5.25 - cut_volume[wall.id()], abs_tol=1e-8), volume


if __name__ == "__main__":
    assert len(sys.argv) > 1, "supply fixtures produced by the Rust integration test"
    for argument in sys.argv[1:]:
        validate(Path(argument))
    print(json.dumps({"validator": ifcopenshell.version, "schema": "IFC4", "schema_errors": 0,
                      "fixtures": len(sys.argv) - 1, "openings": 4 * (len(sys.argv) - 1),
                      "checks": "source identity, relative placement, door hand/swing, OCC void bounds and cut host volume"}))
