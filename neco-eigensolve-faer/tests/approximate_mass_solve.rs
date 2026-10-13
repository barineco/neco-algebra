use neco_eigensolve_faer::{approximate_mass_solve, EigensolveFaerError};
use neco_linear_dense::DenseMatrix;
use neco_linear_types::{LinearError, Shape, Vector};

fn dense(values: Vec<f64>) -> DenseMatrix<f64> {
    DenseMatrix::from_row_major(Shape::new(2, 2), values).expect("matrix")
}

#[test]
fn approximate_mass_solve_matches_diagonal_division() {
    let mass = dense(vec![1.0, 0.0, 0.0, 4.0]);
    let rhs = Vector::try_from_vec(vec![0.2990000000000004, 0.5199999999999998]).expect("rhs");
    let z = approximate_mass_solve(&mass, &rhs).expect("solved");
    assert_eq!(z.values(), &[0.2990000000000004, 0.12999999999999995]);
}

#[test]
fn non_diagonal_mass_matches_independent_gaussian_elimination() {
    let mass = dense(vec![2.0, 1.0, 1.0, 2.0]);
    let rhs = Vector::try_from_vec(vec![3.0, 5.0]).expect("rhs");
    let z = approximate_mass_solve(&mass, &rhs).expect("solved");
    assert!((z.values()[0] - 1.0 / 3.0).abs() <= 1.0e-12);
    assert!((z.values()[1] - 7.0 / 3.0).abs() <= 1.0e-12);
}

#[test]
fn mismatched_rhs_length_is_rejected() {
    let mass = dense(vec![1.0, 0.0, 0.0, 1.0]);
    let rhs = Vector::try_from_vec(vec![1.0]).expect("rhs");
    assert!(matches!(
        approximate_mass_solve(&mass, &rhs),
        Err(EigensolveFaerError::Linear(
            LinearError::StorageLengthMismatch {
                expected: 2,
                actual: 1
            }
        ))
    ));
}

#[test]
fn non_positive_mass_is_rejected() {
    let mass = dense(vec![1.0, 0.0, 0.0, -1.0]);
    let rhs = Vector::try_from_vec(vec![1.0, 1.0]).expect("rhs");
    assert!(matches!(
        approximate_mass_solve(&mass, &rhs),
        Err(EigensolveFaerError::NonPositiveMassMatrix)
    ));
}
