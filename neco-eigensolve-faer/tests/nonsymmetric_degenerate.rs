use neco_complex::Complex;
use neco_eigensolve_faer::{
    condition_number_2, solve_nonsymmetric_f64, ComplexLu, NonsymmetricEigenDecomposition,
};
use neco_linear_dense::DenseMatrix;
use neco_linear_types::Shape;

fn real_matrix(size: usize, values: Vec<f64>) -> DenseMatrix<f64> {
    DenseMatrix::from_column_major(Shape::new(size, size), values).unwrap()
}

fn frobenius_real(matrix: &DenseMatrix<f64>) -> f64 {
    matrix
        .values()
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
}

fn norm(values: &[Complex<f64>]) -> f64 {
    values
        .iter()
        .map(|value| value.norm_squared())
        .sum::<f64>()
        .sqrt()
}

fn column(matrix: &DenseMatrix<Complex<f64>>, index: usize) -> &[Complex<f64>] {
    let rows = matrix.rows();
    &matrix.values()[index * rows..(index + 1) * rows]
}

fn real_times_complex(matrix: &DenseMatrix<f64>, vector: &[Complex<f64>]) -> Vec<Complex<f64>> {
    let size = matrix.rows();
    let mut product = vec![Complex::<f64>::zero(); size];
    for (inner, entry) in vector.iter().enumerate() {
        for (row, target) in product.iter_mut().enumerate() {
            let scale = matrix.values()[inner * size + row];
            *target += Complex::new(scale * entry.real(), scale * entry.imaginary());
        }
    }
    product
}

fn max_residual(matrix: &DenseMatrix<f64>, result: &NonsymmetricEigenDecomposition) -> f64 {
    let scale = frobenius_real(matrix);
    let mut worst: f64 = 0.0;
    for (index, &eigenvalue) in result.eigenvalues().iter().enumerate() {
        let vector = column(result.eigenvectors(), index);
        let product = real_times_complex(matrix, vector);
        let difference: Vec<Complex<f64>> = product
            .iter()
            .zip(vector)
            .map(|(&left, &right)| left - eigenvalue * right)
            .collect();
        worst = worst.max(norm(&difference) / scale);
    }
    worst
}

fn check_structure(result: &NonsymmetricEigenDecomposition, size: usize) {
    let eigenvalues = result.eigenvalues();
    assert_eq!(eigenvalues.len(), size);
    assert_eq!(result.eigenvectors().rows(), size);
    assert_eq!(result.eigenvectors().columns(), size);
    for index in 0..size {
        let vector = column(result.eigenvectors(), index);
        assert!(
            (norm(vector) - 1.0).abs() <= 1e-14,
            "norm of column {index}"
        );
        let moduli: Vec<f64> = vector.iter().map(|value| value.norm()).collect();
        let peak = moduli.iter().copied().fold(0.0, f64::max);
        let pivot = moduli
            .iter()
            .position(|&modulus| modulus >= peak * (1.0 - 1e-12))
            .unwrap();
        assert_eq!(*vector[pivot].imaginary(), 0.0, "phase of column {index}");
        assert!(*vector[pivot].real() > 0.0, "phase of column {index}");
    }
    for pair in eigenvalues.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        assert!(
            right.imaginary() < left.imaginary()
                || (right.imaginary() == left.imaginary() && right.real() <= left.real()),
            "eigenvalue order"
        );
    }
    for index in 0..size {
        let eigenvalue = eigenvalues[index];
        if *eigenvalue.imaginary() == 0.0 {
            assert!(column(result.eigenvectors(), index)
                .iter()
                .all(|value| *value.imaginary() == 0.0));
            continue;
        }
        let vector = column(result.eigenvectors(), index);
        let has_partner = (0..size).any(|other| {
            eigenvalues[other] == eigenvalue.conjugate()
                && column(result.eigenvectors(), other)
                    .iter()
                    .zip(vector)
                    .all(|(partner, own)| *partner == own.conjugate())
        });
        assert!(has_partner, "conjugate partner of column {index}");
    }
}

fn reconstruction_difference(matrix: &DenseMatrix<f64>) -> (f64, f64) {
    let result = solve_nonsymmetric_f64(matrix).unwrap();
    let size = matrix.rows();
    let vectors = result.eigenvectors();
    let mut product = Vec::with_capacity(size * size);
    for index in 0..size {
        product.extend(real_times_complex(matrix, column(vectors, index)));
    }
    let product = DenseMatrix::from_column_major(Shape::new(size, size), product).unwrap();
    let lu = ComplexLu::factor(vectors).unwrap();
    let solved = lu.solve(&product).unwrap();
    let mut sum = 0.0;
    for col in 0..size {
        for row in 0..size {
            let mut expected = Complex::<f64>::zero();
            if row == col {
                expected = result.eigenvalues()[col];
            }
            let difference = solved.values()[col * size + row] - expected;
            sum += difference.norm_squared();
        }
    }
    let kappa = condition_number_2(vectors).unwrap();
    (sum.sqrt() / frobenius_real(matrix), kappa)
}

fn state_space(omega: &[f64], damping: &[f64], coupling: &[f64]) -> DenseMatrix<f64> {
    let n = omega.len();
    let size = 2 * n;
    let mut values = vec![0.0; size * size];
    for index in 0..n {
        values[(n + index) * size + index] = omega[index];
        values[index * size + n + index] = -omega[index];
        for other in 0..n {
            let mut entry = coupling[index] * coupling[other];
            if index == other {
                entry += damping[index];
            }
            values[(n + other) * size + n + index] = -entry;
        }
    }
    real_matrix(size, values)
}

fn oscillators(identical: usize, distinct: usize) -> DenseMatrix<f64> {
    let n = identical + distinct;
    let mut omega = vec![0.0; n];
    let mut damping = vec![0.0; n];
    let mut coupling = vec![0.0; n];
    for index in 0..identical {
        omega[index] = core::f64::consts::TAU * 300.0;
        damping[index] = 0.5;
        coupling[index] = 1.5;
    }
    for index in 0..distinct {
        omega[identical + index] = core::f64::consts::TAU * (280.0 + 45.0 * index as f64);
        damping[identical + index] = 4.0 + index as f64;
        coupling[identical + index] = 2.0 - 0.5 * index as f64;
    }
    state_space(&omega, &damping, &coupling)
}

fn triple_state_space(half: usize) -> DenseMatrix<f64> {
    let mut omega = vec![0.0; half];
    let mut damping = vec![0.0; half];
    let mut coupling = vec![0.0; half];
    for index in 0..half {
        if index < 3 {
            omega[index] = core::f64::consts::TAU * 300.0;
            damping[index] = 0.5;
            coupling[index] = 1.5;
        } else {
            omega[index] = core::f64::consts::TAU * (150.0 + 4.0 * index as f64);
            damping[index] = 0.5 + 0.01 * index as f64;
            coupling[index] = 1.0 + 0.003 * index as f64;
        }
    }
    state_space(&omega, &damping, &coupling)
}

#[test]
fn degenerate_oscillators_meet_the_residual_contract_and_conventions() {
    let cases = [
        ("4 identical", oscillators(4, 0)),
        ("3 identical + 1", oscillators(3, 1)),
        ("3 identical + 3", oscillators(3, 3)),
        ("triple state space n=200", triple_state_space(100)),
    ];
    for (label, matrix) in cases {
        let result = solve_nonsymmetric_f64(&matrix).unwrap();
        check_structure(&result, matrix.rows());
        let residual = max_residual(&matrix, &result);
        assert!(residual <= 1e-12, "{label}: {residual:e}");
    }
}

#[test]
fn refined_eigenvectors_reconstruct_the_matrix_and_stay_independent() {
    let cases = [
        ("triple state space n=200", triple_state_space(100), 2),
        ("4 identical oscillators", oscillators(4, 0), 3),
    ];
    for (label, matrix, multiplicity) in cases {
        let (difference, kappa) = reconstruction_difference(&matrix);
        assert!(kappa.is_finite(), "{label}");
        assert!(difference <= kappa * 1e-12, "{label}: {difference:e}");
        let result = solve_nonsymmetric_f64(&matrix).unwrap();
        let size = matrix.rows();
        let anchor = result.eigenvalues()[group_anchor(&result)];
        let group: Vec<usize> = (0..size)
            .filter(|&index| {
                let value = result.eigenvalues()[index];
                *value.imaginary() > 0.0 && (value - anchor).norm() <= 1e-6 * value.norm()
            })
            .collect();
        assert_eq!(group.len(), multiplicity, "{label}");
        let mut values = Vec::new();
        for &index in &group {
            values.extend_from_slice(column(result.eigenvectors(), index));
        }
        let basis = DenseMatrix::from_column_major(Shape::new(size, group.len()), values).unwrap();
        let condition = group_condition(&basis);
        assert!(condition <= 1e8, "{label}");
    }
}

fn group_anchor(result: &NonsymmetricEigenDecomposition) -> usize {
    let target = core::f64::consts::TAU * 300.0;
    let mut best = 0;
    let mut distance = f64::INFINITY;
    for (index, value) in result.eigenvalues().iter().enumerate() {
        let candidate = (value.imaginary() - target).abs();
        if *value.imaginary() > 0.0 && candidate < distance {
            distance = candidate;
            best = index;
        }
    }
    best
}

fn group_condition(basis: &DenseMatrix<Complex<f64>>) -> f64 {
    let columns = basis.columns();
    let mut gram = vec![Complex::<f64>::zero(); columns * columns];
    for p in 0..columns {
        for q in 0..columns {
            let mut sum = Complex::<f64>::zero();
            for (left, right) in column(basis, p).iter().zip(column(basis, q)) {
                sum += left.conjugate() * *right;
            }
            gram[q * columns + p] = sum;
        }
    }
    let matrix = DenseMatrix::from_column_major(Shape::new(columns, columns), gram).unwrap();
    condition_number_2(&matrix).unwrap().sqrt()
}

fn eigen_bits(result: &NonsymmetricEigenDecomposition) -> Vec<u64> {
    let mut bits = Vec::new();
    for value in result
        .eigenvalues()
        .iter()
        .chain(result.eigenvectors().values())
    {
        bits.push(value.real().to_bits());
        bits.push(value.imaginary().to_bits());
    }
    bits
}

#[test]
fn refined_results_are_independent_of_the_global_parallelism_setting() {
    let matrix = triple_state_space(100);
    let default_first = eigen_bits(&solve_nonsymmetric_f64(&matrix).unwrap());
    let default_second = eigen_bits(&solve_nonsymmetric_f64(&matrix).unwrap());
    faer::disable_global_parallelism();
    let disabled_first = eigen_bits(&solve_nonsymmetric_f64(&matrix).unwrap());
    let disabled_second = eigen_bits(&solve_nonsymmetric_f64(&matrix).unwrap());
    assert_eq!(default_first, default_second);
    assert_eq!(default_first, disabled_first);
    assert_eq!(default_first, disabled_second);
}
