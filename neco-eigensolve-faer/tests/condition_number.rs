use neco_complex::Complex;
use neco_eigensolve_faer::condition_number_2;
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

fn norm(values: &[Complex<f64>]) -> f64 {
    values
        .iter()
        .map(|value| value.norm_squared())
        .sum::<f64>()
        .sqrt()
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

fn diagonal(values: &[Complex<f64>]) -> DenseMatrix<Complex<f64>> {
    let size = values.len();
    let mut storage = vec![Complex::<f64>::zero(); size * size];
    for (index, value) in values.iter().enumerate() {
        storage[index * size + index] = *value;
    }
    DenseMatrix::from_column_major(Shape::new(size, size), storage).unwrap()
}

fn relative_difference(actual: f64, expected: f64) -> f64 {
    (actual - expected).abs() / expected
}

#[test]
fn diagonal_matrix_has_the_ratio_of_its_extreme_moduli() {
    let entries: Vec<Complex<f64>> = (0..40)
        .map(|index| {
            let modulus = 0.25 + 0.5 * index as f64;
            let angle = 0.3 * index as f64;
            Complex::new(modulus * angle.cos(), modulus * angle.sin())
        })
        .collect();
    let expected = (0.25 + 0.5 * 39.0) / 0.25;
    let actual = condition_number_2(&diagonal(&entries)).unwrap();
    assert!(relative_difference(actual, expected) <= 1e-8);
}

#[test]
fn unitary_matrix_has_ratio_one() {
    let unitary = orthonormal_columns(&mut Rng(5), 50);
    let actual = condition_number_2(&unitary).unwrap();
    assert!(relative_difference(actual, 1.0) <= 1e-8);
}

#[test]
fn constructed_ratios_are_recovered() {
    let mut rng = Rng(9);
    for (condition, tolerance) in [(1e5_f64, 1e-8), (1e10_f64, 1e-4)] {
        let matrix = with_condition(&mut rng, 50, condition);
        let actual = condition_number_2(&matrix).unwrap();
        let difference = relative_difference(actual, condition);
        assert!(difference <= tolerance);
    }
}

#[test]
fn singular_matrix_has_infinite_ratio() {
    let entries = [Complex::new(2.0, 1.0), Complex::<f64>::zero()];
    assert_eq!(
        condition_number_2(&diagonal(&entries)).unwrap(),
        f64::INFINITY
    );
}
