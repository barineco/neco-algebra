use neco_complex::Complex;
use neco_eigensolve_faer::{
    condition_number_2, solve_nonsymmetric_f64, ComplexLu, EigensolveFaerError,
};
use neco_linear_dense::DenseMatrix;
use neco_linear_types::Shape;

fn real(rows: usize, columns: usize, values: Vec<f64>) -> DenseMatrix<f64> {
    DenseMatrix::from_column_major(Shape::new(rows, columns), values).unwrap()
}

fn complex(rows: usize, columns: usize, values: Vec<f64>) -> DenseMatrix<Complex<f64>> {
    let values = values
        .into_iter()
        .map(|value| Complex::new(value, 0.0))
        .collect();
    DenseMatrix::from_column_major(Shape::new(rows, columns), values).unwrap()
}

fn identity(size: usize) -> DenseMatrix<Complex<f64>> {
    let mut values = vec![0.0; size * size];
    for index in 0..size {
        values[index * size + index] = 1.0;
    }
    complex(size, size, values)
}

fn assert_invalid<T: core::fmt::Debug>(
    result: Result<T, EigensolveFaerError>,
    expected_operation: &str,
) {
    match result {
        Err(EigensolveFaerError::InvalidMatrix { operation, .. }) => {
            assert_eq!(operation, expected_operation);
        }
        other => panic!("expected InvalidMatrix for {expected_operation}, got {other:?}"),
    }
}

#[test]
fn non_square_matrices_are_invalid() {
    assert_invalid(
        solve_nonsymmetric_f64(&real(2, 3, vec![1.0; 6])),
        "solve_nonsymmetric_f64",
    );
    assert_invalid(
        ComplexLu::factor(&complex(3, 2, vec![1.0; 6])),
        "ComplexLu::factor",
    );
    assert_invalid(
        condition_number_2(&complex(2, 3, vec![1.0; 6])),
        "condition_number_2",
    );
}

#[test]
fn non_finite_elements_are_invalid() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_invalid(
            solve_nonsymmetric_f64(&real(2, 2, vec![1.0, bad, 0.0, 1.0])),
            "solve_nonsymmetric_f64",
        );
        let mut values = identity(2).into_values();
        values[1] = Complex::new(0.0, bad);
        let matrix = DenseMatrix::from_column_major(Shape::new(2, 2), values).unwrap();
        assert_invalid(ComplexLu::factor(&matrix), "ComplexLu::factor");
        assert_invalid(condition_number_2(&matrix), "condition_number_2");
        let lu = ComplexLu::factor(&identity(2)).unwrap();
        assert_invalid(lu.solve(&complex(2, 1, vec![1.0, bad])), "ComplexLu::solve");
    }
}

#[test]
fn empty_matrices_are_invalid() {
    assert_invalid(
        solve_nonsymmetric_f64(&real(0, 0, Vec::new())),
        "solve_nonsymmetric_f64",
    );
    assert_invalid(
        ComplexLu::factor(&complex(0, 0, Vec::new())),
        "ComplexLu::factor",
    );
    assert_invalid(
        condition_number_2(&complex(0, 0, Vec::new())),
        "condition_number_2",
    );
}

#[test]
fn zero_matrix_condition_number_is_invalid() {
    assert_invalid(
        condition_number_2(&complex(3, 3, vec![0.0; 9])),
        "condition_number_2",
    );
}

#[test]
fn zero_matrix_has_a_zero_first_pivot() {
    assert_eq!(
        ComplexLu::factor(&complex(3, 3, vec![0.0; 9])).unwrap_err(),
        EigensolveFaerError::SingularPivot {
            operation: "ComplexLu::factor",
            index: 0,
            magnitude: 0.0
        }
    );
}

#[test]
fn exact_zero_pivot_reports_its_index() {
    let columns = [
        [0.0, -3.0, 0.0, 3.0],
        [-1.0, 1.0, 1.0, -2.0],
        [-1.0, -2.0, 1.0, 1.0],
        [1.0, 0.0, 0.0, 3.0],
    ];
    let values = columns.iter().flatten().copied().collect();
    assert_eq!(
        ComplexLu::factor(&complex(4, 4, values)).unwrap_err(),
        EigensolveFaerError::SingularPivot {
            operation: "ComplexLu::factor",
            index: 2,
            magnitude: 0.0
        }
    );
}

#[test]
fn right_hand_side_row_count_must_match() {
    let lu = ComplexLu::factor(&identity(4)).unwrap();
    assert_eq!(
        lu.solve(&complex(3, 1, vec![1.0; 3])).unwrap_err(),
        EigensolveFaerError::RightHandSide {
            operation: "ComplexLu::solve",
            expected: 4,
            actual: 3
        }
    );
}

#[test]
fn right_hand_side_without_columns_gives_a_solution_without_columns() {
    let lu = ComplexLu::factor(&identity(4)).unwrap();
    let solution = lu.solve(&complex(4, 0, Vec::new())).unwrap();
    assert_eq!(solution.rows(), 4);
    assert_eq!(solution.columns(), 0);
}

#[test]
fn extreme_element_magnitudes_are_solved_by_scaling() {
    for scale in [1e300, 1e-300, 1e150, 1e-150] {
        let eigen =
            solve_nonsymmetric_f64(&real(2, 2, vec![scale, 0.0, 0.0, 2.0 * scale])).unwrap();
        assert_eq!(eigen.eigenvalues()[0].real_value(), 2.0 * scale);
        assert_eq!(eigen.eigenvalues()[1].real_value(), scale);
        let matrix = complex(2, 2, vec![scale, 0.0, 0.0, scale]);
        assert_eq!(condition_number_2(&matrix).unwrap(), 1.0);
        let lu = ComplexLu::factor(&matrix).unwrap();
        lu.solve(&complex(2, 1, vec![1.0, 1.0])).unwrap();
    }
}

#[test]
fn overflowing_eigenvalues_are_reported() {
    let huge = 1.7e308;
    assert_eq!(
        solve_nonsymmetric_f64(&real(2, 2, vec![huge; 4])).unwrap_err(),
        EigensolveFaerError::NonFiniteResult {
            operation: "solve_nonsymmetric_f64"
        }
    );
}

#[test]
fn overflowing_solutions_are_reported() {
    let lu = ComplexLu::factor(&complex(2, 2, vec![1.0, 0.0, 0.0, 1e-300])).unwrap();
    assert_eq!(
        lu.solve(&complex(2, 1, vec![1.0, 1e150])).unwrap_err(),
        EigensolveFaerError::NonFiniteResult {
            operation: "ComplexLu::solve"
        }
    );
}
