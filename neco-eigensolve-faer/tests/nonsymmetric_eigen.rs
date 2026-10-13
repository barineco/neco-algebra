use neco_complex::Complex;
use neco_eigensolve_faer::{
    condition_number_2, solve_nonsymmetric_f64, ComplexLu, NonsymmetricEigenDecomposition,
};
use neco_linear_dense::DenseMatrix;
use neco_linear_types::Shape;

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

fn real_matrix(size: usize, values: Vec<f64>) -> DenseMatrix<f64> {
    DenseMatrix::from_column_major(Shape::new(size, size), values).unwrap()
}

fn random_real(rng: &mut Rng, size: usize) -> DenseMatrix<f64> {
    real_matrix(size, (0..size * size).map(|_| rng.unit()).collect())
}

fn random_complex(rng: &mut Rng, rows: usize, columns: usize) -> DenseMatrix<Complex<f64>> {
    let values = (0..rows * columns)
        .map(|_| Complex::new(rng.unit(), rng.unit()))
        .collect();
    DenseMatrix::from_column_major(Shape::new(rows, columns), values).unwrap()
}

fn state_space(rng: &mut Rng, half: usize) -> DenseMatrix<f64> {
    let size = 2 * half;
    let frequencies: Vec<f64> = (0..half)
        .map(|index| {
            if index < 3 {
                1.5
            } else {
                1.0 + 0.37 * index as f64
            }
        })
        .collect();
    let factor: Vec<f64> = (0..half * half).map(|_| rng.unit()).collect();
    let mut values = vec![0.0; size * size];
    for index in 0..half {
        values[(half + index) * size + index] = frequencies[index];
        values[index * size + half + index] = -frequencies[index];
    }
    for row in 0..half {
        for column in 0..half {
            let mut sum = 0.0;
            for inner in 0..half {
                sum += factor[inner * half + row] * factor[inner * half + column];
            }
            values[(half + column) * size + half + row] = -0.5 * sum / half as f64;
        }
    }
    real_matrix(size, values)
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

fn check_trace_and_conditioning(
    matrix: &DenseMatrix<f64>,
    result: &NonsymmetricEigenDecomposition,
) {
    let size = matrix.rows();
    let trace: f64 = (0..size)
        .map(|index| matrix.values()[index * size + index])
        .sum();
    let sum = result
        .eigenvalues()
        .iter()
        .fold(Complex::<f64>::zero(), |total, &value| total + value);
    let difference = (sum - Complex::new(trace, 0.0)).norm();
    assert!(
        difference <= 1e-12 * frobenius_real(matrix),
        "trace difference {difference:e}"
    );
    let kappa = condition_number_2(result.eigenvectors()).unwrap();
    assert!(kappa.is_finite(), "kappa of the eigenvector matrix");
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

fn expected_oscillator_eigenvalues(frequencies: &[f64], dampings: &[f64]) -> Vec<Complex<f64>> {
    let mut expected = Vec::new();
    for (&omega, &damping) in frequencies.iter().zip(dampings) {
        let imaginary = (omega * omega - damping * damping / 4.0).sqrt();
        expected.push(Complex::new(-damping / 2.0, imaginary));
        expected.push(Complex::new(-damping / 2.0, -imaginary));
    }
    expected.sort_by(|left, right| {
        right
            .imaginary()
            .partial_cmp(left.imaginary())
            .unwrap()
            .then(right.real().partial_cmp(left.real()).unwrap())
    });
    expected
}

#[test]
fn damped_oscillators_match_closed_form_eigenvalues() {
    for half in [1usize, 8, 64] {
        let frequencies: Vec<f64> = (0..half).map(|index| 1.0 + 0.5 * index as f64).collect();
        let dampings: Vec<f64> = (0..half).map(|index| 0.05 + 0.003 * index as f64).collect();
        let size = 2 * half;
        let mut values = vec![0.0; size * size];
        for index in 0..half {
            values[(half + index) * size + index] = frequencies[index];
            values[index * size + half + index] = -frequencies[index];
            values[(half + index) * size + half + index] = -dampings[index];
        }
        let result = solve_nonsymmetric_f64(&real_matrix(size, values)).unwrap();
        let expected = expected_oscillator_eigenvalues(&frequencies, &dampings);
        let mut worst: f64 = 0.0;
        for (actual, wanted) in result.eigenvalues().iter().zip(&expected) {
            worst = worst.max((*actual - *wanted).norm() / wanted.norm());
        }
        assert!(worst <= 1e-12, "half {half}: {worst:e}");
    }
}

#[test]
fn general_matrices_satisfy_residual_and_normalization() {
    let mut rng = Rng(11);
    for size in [1usize, 2, 7, 100, 400] {
        let matrix = random_real(&mut rng, size);
        let result = solve_nonsymmetric_f64(&matrix).unwrap();
        check_structure(&result, size);
        check_trace_and_conditioning(&matrix, &result);
        let residual = max_residual(&matrix, &result);
        assert!(residual <= 1e-12, "n {size}: {residual:e}");
    }
    let matrix = state_space(&mut rng, 100);
    let result = solve_nonsymmetric_f64(&matrix).unwrap();
    check_structure(&result, 200);
    check_trace_and_conditioning(&matrix, &result);
    let residual = max_residual(&matrix, &result);
    assert!(residual <= 1e-12, "state space: {residual:e}");
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

#[test]
fn eigenvectors_reconstruct_the_matrix() {
    let mut rng = Rng(23);
    let cases = [
        ("random n=100", random_real(&mut rng, 100)),
        ("state space n=200", state_space(&mut rng, 100)),
    ];
    for (label, matrix) in cases {
        let (difference, kappa) = reconstruction_difference(&matrix);
        assert!(kappa.is_finite(), "{label}: kappa");
        assert!(difference <= kappa * 1e-12, "{label}: {difference:e}");
    }
}

fn complex_times(
    matrix: &DenseMatrix<Complex<f64>>,
    other: &DenseMatrix<Complex<f64>>,
) -> DenseMatrix<Complex<f64>> {
    let (rows, inner, columns) = (matrix.rows(), matrix.columns(), other.columns());
    let mut values = vec![Complex::<f64>::zero(); rows * columns];
    for col in 0..columns {
        for k in 0..inner {
            let factor = other.values()[col * inner + k];
            for row in 0..rows {
                values[col * rows + row] += matrix.values()[k * rows + row] * factor;
            }
        }
    }
    DenseMatrix::from_column_major(Shape::new(rows, columns), values).unwrap()
}

fn orthonormal_columns(rng: &mut Rng, size: usize) -> DenseMatrix<Complex<f64>> {
    let mut columns: Vec<Vec<Complex<f64>>> = Vec::new();
    for _ in 0..size {
        let mut vector: Vec<Complex<f64>> = (0..size)
            .map(|_| Complex::new(rng.unit(), rng.unit()))
            .collect();
        for _ in 0..2 {
            for previous in &columns {
                let mut projection = Complex::<f64>::zero();
                for (own, other) in vector.iter().zip(previous) {
                    projection += other.conjugate() * *own;
                }
                for (own, other) in vector.iter_mut().zip(previous) {
                    *own -= projection * *other;
                }
            }
        }
        let length = norm(&vector);
        for value in &mut vector {
            *value = Complex::new(value.real() / length, value.imaginary() / length);
        }
        columns.push(vector);
    }
    DenseMatrix::from_column_major(
        Shape::new(size, size),
        columns.into_iter().flatten().collect(),
    )
    .unwrap()
}

fn with_condition(rng: &mut Rng, size: usize, condition: f64) -> DenseMatrix<Complex<f64>> {
    let left = orthonormal_columns(rng, size);
    let right = orthonormal_columns(rng, size);
    let mut scaled = left.values().to_vec();
    for col in 0..size {
        let sigma = condition.powf(-(col as f64) / (size.max(2) - 1) as f64);
        for row in 0..size {
            let value = scaled[col * size + row];
            scaled[col * size + row] =
                Complex::new(value.real() * sigma, value.imaginary() * sigma);
        }
    }
    let scaled = DenseMatrix::from_column_major(Shape::new(size, size), scaled).unwrap();
    let mut adjoint = vec![Complex::<f64>::zero(); size * size];
    for row in 0..size {
        for col in 0..size {
            adjoint[col * size + row] = right.values()[row * size + col].conjugate();
        }
    }
    let adjoint = DenseMatrix::from_column_major(Shape::new(size, size), adjoint).unwrap();
    complex_times(&scaled, &adjoint)
}

fn frobenius_complex(matrix: &DenseMatrix<Complex<f64>>) -> f64 {
    norm(matrix.values())
}

fn check_lu_solve(label: &str, matrix: &DenseMatrix<Complex<f64>>, rng: &mut Rng) {
    let size = matrix.rows();
    let truth = random_complex(rng, size, 3);
    let rhs = complex_times(matrix, &truth);
    let lu = ComplexLu::factor(matrix).unwrap();
    let solution = lu.solve(&rhs).unwrap();
    let kappa = condition_number_2(matrix).unwrap();
    let mut worst_residual: f64 = 0.0;
    let mut worst_forward: f64 = 0.0;
    for col in 0..3 {
        let y = column(&solution, col);
        let b = column(&rhs, col);
        let x = column(&truth, col);
        let single = DenseMatrix::from_column_major(Shape::new(size, 1), y.to_vec()).unwrap();
        let reproduced = complex_times(matrix, &single);
        let difference: Vec<Complex<f64>> = reproduced
            .values()
            .iter()
            .zip(b)
            .map(|(&left, &right)| left - right)
            .collect();
        let residual = norm(&difference) / (frobenius_complex(matrix) * norm(y) + norm(b));
        let error: Vec<Complex<f64>> = y
            .iter()
            .zip(x)
            .map(|(&left, &right)| left - right)
            .collect();
        worst_residual = worst_residual.max(residual);
        worst_forward = worst_forward.max(norm(&error) / norm(x));
    }
    assert!(kappa.is_finite(), "{label}: kappa");
    assert!(
        worst_residual <= 1e-12,
        "{label}: residual {worst_residual:e}"
    );
    assert!(
        worst_forward <= kappa * 1e-12,
        "{label}: forward {worst_forward:e}"
    );
}

#[test]
fn lu_solve_has_small_residual_and_forward_error() {
    let mut rng = Rng(37);
    for size in [1usize, 50, 300] {
        let matrix = random_complex(&mut rng, size, size);
        check_lu_solve(&format!("random n={size}"), &matrix, &mut rng);
    }
    let matrix = with_condition(&mut rng, 50, 1e5);
    check_lu_solve("condition 1e5 n=50", &matrix, &mut rng);
}

#[test]
fn factorization_is_reused_for_several_right_hand_sides() {
    let mut rng = Rng(41);
    let matrix = random_complex(&mut rng, 20, 20);
    let lu = ComplexLu::factor(&matrix).unwrap();
    let first = random_complex(&mut rng, 20, 2);
    let second = random_complex(&mut rng, 20, 5);
    let first_again = lu.solve(&first).unwrap();
    lu.solve(&second).unwrap();
    assert_eq!(lu.solve(&first).unwrap(), first_again);
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

fn solve_bits(matrix: &DenseMatrix<Complex<f64>>, rhs: &DenseMatrix<Complex<f64>>) -> Vec<u64> {
    let solution = ComplexLu::factor(matrix).unwrap().solve(rhs).unwrap();
    let mut bits: Vec<u64> = solution
        .values()
        .iter()
        .flat_map(|value| [value.real().to_bits(), value.imaginary().to_bits()])
        .collect();
    bits.push(condition_number_2(matrix).unwrap().to_bits());
    bits
}

#[test]
fn results_are_independent_of_the_global_parallelism_setting() {
    let matrix = random_real(&mut Rng(53), 100);
    let mut rng = Rng(59);
    let complex_matrix = random_complex(&mut rng, 50, 50);
    let rhs = random_complex(&mut rng, 50, 3);
    let default_first = eigen_bits(&solve_nonsymmetric_f64(&matrix).unwrap());
    let default_second = eigen_bits(&solve_nonsymmetric_f64(&matrix).unwrap());
    let default_solve = solve_bits(&complex_matrix, &rhs);
    faer::disable_global_parallelism();
    let disabled_first = eigen_bits(&solve_nonsymmetric_f64(&matrix).unwrap());
    let disabled_second = eigen_bits(&solve_nonsymmetric_f64(&matrix).unwrap());
    let disabled_solve = solve_bits(&complex_matrix, &rhs);
    assert_eq!(default_first, default_second);
    assert_eq!(default_first, disabled_first);
    assert_eq!(default_first, disabled_second);
    assert_eq!(default_solve, disabled_solve);
    assert_eq!(default_solve, solve_bits(&complex_matrix, &rhs));
}

fn scaled_real(matrix: &DenseMatrix<f64>, factor: f64) -> DenseMatrix<f64> {
    DenseMatrix::from_column_major(
        matrix.shape(),
        matrix.values().iter().map(|value| value * factor).collect(),
    )
    .unwrap()
}

fn scaled_complex(matrix: &DenseMatrix<Complex<f64>>, factor: f64) -> DenseMatrix<Complex<f64>> {
    DenseMatrix::from_column_major(
        matrix.shape(),
        matrix
            .values()
            .iter()
            .map(|value| Complex::new(value.real() * factor, value.imaginary() * factor))
            .collect(),
    )
    .unwrap()
}

fn to_complex(matrix: &DenseMatrix<f64>) -> DenseMatrix<Complex<f64>> {
    DenseMatrix::from_column_major(
        matrix.shape(),
        matrix
            .values()
            .iter()
            .map(|&value| Complex::new(value, 0.0))
            .collect(),
    )
    .unwrap()
}

fn eigen_residual_in_original_scale(
    matrix: &DenseMatrix<f64>,
    result: &NonsymmetricEigenDecomposition,
    factor: f64,
) -> f64 {
    let scale = frobenius_real(matrix);
    let mut worst: f64 = 0.0;
    for (index, &eigenvalue) in result.eigenvalues().iter().enumerate() {
        let original = Complex::new(eigenvalue.real() / factor, eigenvalue.imaginary() / factor);
        let vector = column(result.eigenvectors(), index);
        let difference: Vec<Complex<f64>> = real_times_complex(matrix, vector)
            .iter()
            .zip(vector)
            .map(|(&left, &right)| left - original * right)
            .collect();
        worst = worst.max(norm(&difference) / scale);
    }
    worst
}

#[test]
fn scaled_inputs_keep_residuals_and_condition_numbers() {
    let mut rng = Rng(71);
    let random = random_real(&mut rng, 60);
    let state = state_space(&mut rng, 100);
    let complex_matrix = random_complex(&mut rng, 60, 60);
    let truth = random_complex(&mut rng, 60, 3);
    let rhs = complex_times(&complex_matrix, &truth);
    let reference_random = condition_number_2(&complex_matrix).unwrap();
    let reference_state = condition_number_2(&to_complex(&state)).unwrap();
    for factor in [1e-300, 1e-13, 1e140, 1e300] {
        for (label, base) in [("random n=60", &random), ("state space n=200", &state)] {
            let result = solve_nonsymmetric_f64(&scaled_real(base, factor)).unwrap();
            let residual = eigen_residual_in_original_scale(base, &result, factor);
            assert!(residual <= 1e-12, "{label} {factor:e}: {residual:e}");
        }
        let scaled = scaled_complex(&complex_matrix, factor);
        let solution = ComplexLu::factor(&scaled).unwrap().solve(&rhs).unwrap();
        let mut worst: f64 = 0.0;
        for col in 0..3 {
            let rescaled: Vec<Complex<f64>> = column(&solution, col)
                .iter()
                .map(|value| Complex::new(value.real() * factor, value.imaginary() * factor))
                .collect();
            let single =
                DenseMatrix::from_column_major(Shape::new(60, 1), rescaled.clone()).unwrap();
            let difference: Vec<Complex<f64>> = complex_times(&complex_matrix, &single)
                .values()
                .iter()
                .zip(column(&rhs, col))
                .map(|(&left, &right)| left - right)
                .collect();
            worst = worst.max(
                norm(&difference)
                    / (frobenius_complex(&complex_matrix) * norm(&rescaled)
                        + norm(column(&rhs, col))),
            );
        }
        assert!(worst <= 1e-12, "lu {factor:e}: {worst:e}");
        for (label, base, reference) in [
            ("random", complex_matrix.clone(), reference_random),
            ("state space", to_complex(&state), reference_state),
        ] {
            let value = condition_number_2(&scaled_complex(&base, factor)).unwrap();
            let difference = (value - reference).abs() / reference;
            assert!(
                difference <= 1e-12,
                "condition {label} {factor:e}: {difference:e}"
            );
        }
    }
}

#[test]
fn diagonal_systems_with_a_wide_dynamic_range_are_solved_exactly() {
    for (first, second) in [(1e200, 1e-200), (1e10, 1e-300)] {
        let matrix = DenseMatrix::from_column_major(
            Shape::new(2, 2),
            vec![
                Complex::new(first, 0.0),
                Complex::<f64>::zero(),
                Complex::<f64>::zero(),
                Complex::new(second, 0.0),
            ],
        )
        .unwrap();
        let rhs = DenseMatrix::from_column_major(
            Shape::new(2, 1),
            vec![Complex::new(1.0, 0.0), Complex::new(1.0, 0.0)],
        )
        .unwrap();
        let solution = ComplexLu::factor(&matrix).unwrap().solve(&rhs).unwrap();
        for (value, divisor) in solution.values().iter().zip([first, second]) {
            let expected = 1.0 / divisor;
            let difference = (value.real() - expected).abs() / expected;
            assert!(difference <= 1e-15, "{first:e} {second:e}: {difference:e}");
            assert_eq!(*value.imaginary(), 0.0);
        }
    }
}
