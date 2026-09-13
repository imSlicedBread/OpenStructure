use os_core::Point2;
use os_ifc::{IfcAdapter, WallIfc};
use os_model::{Grid, GridParams, Model};

#[test]
fn grids_are_explicitly_reported_as_exchange_loss() {
    let mut model = Model::new("Grid exchange");
    model.views.clear(); // Ensure the grid alone produces the loss warning.
    let grid = Grid::new(
        "core.grid",
        GridParams {
            name: "A".into(),
            building: *model.buildings.keys().next().unwrap(),
            start: Point2::new(0.0, 0.0),
            end: Point2::new(10.0, 0.0),
        },
    );
    model.grids.insert(grid.id(), grid);
    let before = model.clone();
    assert!(WallIfc.export(&model).is_err());
    let exchange = WallIfc.export_report(&model).unwrap();
    assert!(
        exchange
            .warnings
            .iter()
            .any(|warning| warning.contains("1 native architectural grids"))
    );
    assert_eq!(model, before);
    let imported = WallIfc.import_report(&exchange.value).unwrap();
    assert!(imported.value.grids.is_empty());
}
