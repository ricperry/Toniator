use toniator_geometry::{NominalCellBasis, Vector2};

/// Proves the nominal cell diameter uses the longer signed diagonal rather than an axis length.
#[test]
fn nominal_cell_basis_uses_the_longer_parallelogram_diagonal() {
    let basis = NominalCellBasis::new(Vector2::new(3.0, 0.0), Vector2::new(0.0, 4.0))
        .expect("finite nonzero axes form a nominal basis");

    assert_eq!(basis.diameter(), 5.0);
}

/// Proves a nominal basis rejects non-finite and degenerate axes before family publication.
#[test]
fn nominal_cell_basis_rejects_invalid_axes() {
    assert!(NominalCellBasis::new(Vector2::new(0.0, 0.0), Vector2::new(1.0, 0.0)).is_err());
    assert!(NominalCellBasis::new(Vector2::new(f64::NAN, 0.0), Vector2::new(1.0, 0.0)).is_err());
}
