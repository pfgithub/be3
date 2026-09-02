use super::*;

#[test]
fn compiling_an_empty_grid_reports_an_error() {
    let error = generate_initial(Uuid::new_v4(), &LogicGrid::new()).unwrap_err();

    assert_eq!(error, "An empty grid cannot be compiled");
}
