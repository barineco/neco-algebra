use crate::convert::{
    real_scale_exponent, scale_by_power_of_two, scaled_real_values, validate_square_real,
};
use crate::refine::certify_and_refine;
use crate::{try_filled, try_workspace, EigensolveFaerError};
use faer::diag::DiagMut;
use faer::dyn_stack::MemStack;
use faer::linalg::evd::{evd_real, evd_scratch, ComputeEigenvectors};
use faer::{MatMut, MatRef, Par, Spec};
use neco_complex::Complex;
use neco_linear_dense::DenseMatrix;
use neco_linear_types::{LinearError, Shape};

const OPERATION: &str = "solve_nonsymmetric_f64";

#[derive(Clone, Debug, PartialEq)]
pub struct NonsymmetricEigenDecomposition {
    eigenvalues: Vec<Complex<f64>>,
    eigenvectors: DenseMatrix<Complex<f64>>,
}

impl NonsymmetricEigenDecomposition {
    pub fn eigenvalues(&self) -> &[Complex<f64>] {
        &self.eigenvalues
    }

    pub fn eigenvectors(&self) -> &DenseMatrix<Complex<f64>> {
        &self.eigenvectors
    }
}

pub fn solve_nonsymmetric_f64(
    matrix: &DenseMatrix<f64>,
) -> Result<NonsymmetricEigenDecomposition, EigensolveFaerError> {
    let mut raw = decompose_matrix(matrix)?;
    raw.refine()?;
    raw.into_sorted()
}

struct Decomposition {
    dimension: usize,
    exponent: i32,
    scaled: Vec<f64>,
    eigenvalues: Vec<Complex<f64>>,
    vectors: Vec<Complex<f64>>,
    partner: Vec<usize>,
    order: Vec<usize>,
}

fn decompose_matrix(matrix: &DenseMatrix<f64>) -> Result<Decomposition, EigensolveFaerError> {
    let dimension = validate_square_real(matrix, OPERATION)?;
    let elements = dimension
        .checked_mul(dimension)
        .ok_or(LinearError::CapacityOverflow {
            requested: usize::MAX,
        })?;
    let exponent = real_scale_exponent(matrix.values());
    let scaled = scaled_real_values(matrix.values(), -exponent)?;
    let mut real_parts = try_filled(dimension, 0.0)?;
    let mut imaginary_parts = try_filled(dimension, 0.0)?;
    let mut real_vectors = try_filled(elements, 0.0)?;
    decompose(
        &scaled,
        &mut real_parts,
        &mut imaginary_parts,
        &mut real_vectors,
    )?;
    let mut eigenvalues = try_filled(dimension, Complex::<f64>::zero())?;
    let mut vectors = try_filled(elements, Complex::<f64>::zero())?;
    let mut partner: Vec<usize> = try_filled(dimension, 0)?;
    expand_pairs(
        &real_parts,
        &imaginary_parts,
        &real_vectors,
        exponent,
        &mut eigenvalues,
        &mut vectors,
        &mut partner,
    )?;
    let mut order: Vec<usize> = try_filled(dimension, 0)?;
    for (position, slot) in order.iter_mut().enumerate() {
        *slot = position;
    }
    order.sort_unstable_by(|&left, &right| {
        descending_order(&eigenvalues[left], &eigenvalues[right]).then(left.cmp(&right))
    });
    Ok(Decomposition {
        dimension,
        exponent,
        scaled,
        eigenvalues,
        vectors,
        partner,
        order,
    })
}

impl Decomposition {
    fn refine(&mut self) -> Result<(), EigensolveFaerError> {
        let mut rank: Vec<usize> = try_filled(self.dimension, 0)?;
        for (position, &source) in self.order.iter().enumerate() {
            rank[source] = position;
        }
        let mut scaled_eigenvalues = try_filled(self.dimension, Complex::<f64>::zero())?;
        for (slot, value) in scaled_eigenvalues.iter_mut().zip(&self.eigenvalues) {
            *slot = Complex::new(
                scale_by_power_of_two(*value.real(), -self.exponent),
                scale_by_power_of_two(*value.imaginary(), -self.exponent),
            );
        }
        certify_and_refine(
            &self.scaled,
            &scaled_eigenvalues,
            &self.partner,
            &mut self.vectors,
            &rank,
            OPERATION,
        )
    }

    fn into_sorted(self) -> Result<NonsymmetricEigenDecomposition, EigensolveFaerError> {
        let dimension = self.dimension;
        let mut sorted_eigenvalues = try_filled(dimension, Complex::<f64>::zero())?;
        let mut sorted_vectors = try_filled(self.vectors.len(), Complex::<f64>::zero())?;
        for (position, &source) in self.order.iter().enumerate() {
            sorted_eigenvalues[position] = self.eigenvalues[source];
            sorted_vectors[position * dimension..(position + 1) * dimension]
                .copy_from_slice(&self.vectors[source * dimension..(source + 1) * dimension]);
        }
        Ok(NonsymmetricEigenDecomposition {
            eigenvalues: sorted_eigenvalues,
            eigenvectors: DenseMatrix::from_column_major(
                Shape::new(dimension, dimension),
                sorted_vectors,
            )?,
        })
    }
}

#[cfg(test)]
fn solve_without_refinement(
    matrix: &DenseMatrix<f64>,
) -> Result<NonsymmetricEigenDecomposition, EigensolveFaerError> {
    decompose_matrix(matrix)?.into_sorted()
}

fn descending_order(left: &Complex<f64>, right: &Complex<f64>) -> core::cmp::Ordering {
    use core::cmp::Ordering::Equal;
    right
        .imaginary()
        .partial_cmp(left.imaginary())
        .unwrap_or(Equal)
        .then_with(|| right.real().partial_cmp(left.real()).unwrap_or(Equal))
}

fn decompose(
    values: &[f64],
    real_parts: &mut [f64],
    imaginary_parts: &mut [f64],
    vectors: &mut [f64],
) -> Result<(), EigensolveFaerError> {
    let dimension = real_parts.len();
    let params = Spec::default();
    let mut workspace = try_workspace(evd_scratch::<f64>(
        dimension,
        ComputeEigenvectors::No,
        ComputeEigenvectors::Yes,
        Par::Seq,
        params,
    ))?;
    evd_real(
        MatRef::from_column_major_slice(values, dimension, dimension),
        DiagMut::from_slice_mut(real_parts),
        DiagMut::from_slice_mut(imaginary_parts),
        None,
        Some(MatMut::from_column_major_slice_mut(
            vectors, dimension, dimension,
        )),
        Par::Seq,
        MemStack::new(&mut workspace),
        params,
    )
    .map_err(|_| EigensolveFaerError::ExternalSolver {
        operation: OPERATION,
        reason: "nonsymmetric eigendecomposition did not converge",
    })
}

/// faer stores a complex pair in two adjacent real columns: the first holds the
/// real part and the second the imaginary part of the eigenvector.
fn expand_pairs(
    real_parts: &[f64],
    imaginary_parts: &[f64],
    real_vectors: &[f64],
    exponent: i32,
    eigenvalues: &mut [Complex<f64>],
    vectors: &mut [Complex<f64>],
    partner: &mut [usize],
) -> Result<(), EigensolveFaerError> {
    let dimension = real_parts.len();
    let mut moduli = try_filled(dimension, 0.0)?;
    let mut column = 0;
    while column < dimension {
        let real = scale_by_power_of_two(real_parts[column], exponent);
        let imaginary = scale_by_power_of_two(imaginary_parts[column], exponent);
        if !real.is_finite() || !imaginary.is_finite() {
            return Err(EigensolveFaerError::NonFiniteResult {
                operation: OPERATION,
            });
        }
        let first = &real_vectors[column * dimension..(column + 1) * dimension];
        if imaginary == 0.0 {
            eigenvalues[column] = Complex::new(real, imaginary);
            partner[column] = column;
            let target = &mut vectors[column * dimension..(column + 1) * dimension];
            for (slot, &value) in target.iter_mut().zip(first) {
                *slot = Complex::new(value, 0.0);
            }
            normalize(target, &mut moduli)?;
            column += 1;
        } else {
            let Some(second) = real_vectors.get((column + 1) * dimension..(column + 2) * dimension)
            else {
                return Err(EigensolveFaerError::ExternalSolver {
                    operation: OPERATION,
                    reason: "complex eigenvalue lacks a partner column",
                });
            };
            eigenvalues[column] = Complex::new(real, imaginary);
            eigenvalues[column + 1] = Complex::new(real, -imaginary);
            partner[column] = column + 1;
            partner[column + 1] = column;
            let (head, tail) = vectors.split_at_mut((column + 1) * dimension);
            let own = &mut head[column * dimension..];
            let partner = &mut tail[..dimension];
            let (positive, negative) = if imaginary > 0.0 {
                (own, partner)
            } else {
                (partner, own)
            };
            for ((slot, &re), &im) in positive.iter_mut().zip(first).zip(second) {
                *slot = Complex::new(re, if imaginary > 0.0 { im } else { -im });
            }
            normalize(positive, &mut moduli)?;
            for (slot, value) in negative.iter_mut().zip(positive.iter()) {
                *slot = value.conjugate();
            }
            column += 2;
        }
    }
    Ok(())
}

pub(crate) fn normalize(
    vector: &mut [Complex<f64>],
    moduli: &mut [f64],
) -> Result<(), EigensolveFaerError> {
    let non_finite = EigensolveFaerError::NonFiniteResult {
        operation: OPERATION,
    };
    let largest = vector.iter().map(|value| value.norm()).fold(0.0, f64::max);
    if !largest.is_finite() {
        return Err(non_finite);
    }
    if largest == 0.0 {
        return Err(EigensolveFaerError::ExternalSolver {
            operation: OPERATION,
            reason: "eigenvector is zero",
        });
    }
    for value in vector.iter_mut() {
        *value = Complex::new(value.real() / largest, value.imaginary() / largest);
    }
    let norm = vector
        .iter()
        .map(|value| value.norm_squared())
        .sum::<f64>()
        .sqrt();
    for value in vector.iter_mut() {
        *value = Complex::new(value.real() / norm, value.imaginary() / norm);
    }
    for (modulus, value) in moduli.iter_mut().zip(vector.iter()) {
        *modulus = value.norm();
    }
    let peak = moduli.iter().copied().fold(0.0, f64::max);
    let threshold = peak * (1.0 - 1e-12);
    let Some(index) = moduli.iter().position(|&modulus| modulus >= threshold) else {
        return Err(non_finite);
    };
    let pivot = vector[index];
    let modulus = moduli[index];
    let phase = Complex::new(pivot.real() / modulus, -pivot.imaginary() / modulus);
    for value in vector.iter_mut() {
        *value *= phase;
    }
    vector[index] = Complex::new(modulus, 0.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalized(parts: &[(f64, f64)]) -> Vec<Complex<f64>> {
        let mut vector: Vec<Complex<f64>> =
            parts.iter().map(|&(re, im)| Complex::new(re, im)).collect();
        let mut moduli = vec![0.0; vector.len()];
        normalize(&mut vector, &mut moduli).unwrap();
        vector
    }

    #[test]
    fn phase_makes_first_near_maximal_component_positive_real() {
        let vector = normalized(&[(0.0, 3.0), (4.0, 0.0)]);
        let norm: f64 = vector.iter().map(|value| value.norm_squared()).sum::<f64>();
        assert!((norm - 1.0).abs() < 1e-15);
        assert_eq!(*vector[1].imaginary(), 0.0);
        assert!((vector[1].real() - 0.8).abs() < 1e-15);
        assert!((vector[0].real() - 0.0).abs() < 1e-15);
        assert!((vector[0].imaginary() - 0.6).abs() < 1e-15);
    }

    #[test]
    fn tie_within_tolerance_selects_smallest_index() {
        let vector = normalized(&[(1.0, 0.0), (0.0, 1.0 - 1e-13)]);
        assert_eq!(*vector[0].imaginary(), 0.0);
        assert!(*vector[0].real() > 0.0);
        assert!(vector[1].real().abs() < 1e-12);
    }

    #[test]
    fn real_vector_sign_follows_first_component() {
        let vector = normalized(&[(-2.0, 0.0), (1.0, 0.0)]);
        assert!(*vector[0].real() > 0.0);
        assert!(*vector[1].real() < 0.0);
    }

    #[test]
    fn conjugate_pair_keeps_faer_order_and_conjugate_vectors() {
        let real_parts = [1.0, 1.0];
        let imaginary_parts = [-2.0, 2.0];
        let real_vectors = [1.0, 0.0, 0.0, 1.0];
        let mut eigenvalues = [Complex::<f64>::zero(); 2];
        let mut vectors = [Complex::<f64>::zero(); 4];
        let mut partner = [0usize; 2];
        expand_pairs(
            &real_parts,
            &imaginary_parts,
            &real_vectors,
            0,
            &mut eigenvalues,
            &mut vectors,
            &mut partner,
        )
        .unwrap();
        assert_eq!(*eigenvalues[0].imaginary(), -2.0);
        assert_eq!(*eigenvalues[1].imaginary(), 2.0);
        for (lower, upper) in vectors[0..2].iter().zip(&vectors[2..4]) {
            assert_eq!(upper.conjugate(), *lower);
        }
        assert!(*vectors[2].real() > 0.0);
        assert_eq!(*vectors[2].imaginary(), 0.0);
    }

    fn oscillators(strings: usize, plates: usize) -> DenseMatrix<f64> {
        let n = strings + plates;
        let mut omega = vec![0.0; n];
        let mut damping = vec![0.0; n];
        let mut coupling = vec![0.0; n];
        for index in 0..strings {
            omega[index] = core::f64::consts::TAU * 300.0;
            damping[index] = 0.5;
            coupling[index] = 1.5;
        }
        for index in 0..plates {
            omega[strings + index] = core::f64::consts::TAU * (280.0 + 45.0 * index as f64);
            damping[strings + index] = 4.0 + index as f64;
            coupling[strings + index] = 2.0 - 0.5 * index as f64;
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
        DenseMatrix::from_column_major(Shape::new(size, size), values).unwrap()
    }

    fn worst_ratio(matrix: &DenseMatrix<f64>, result: &NonsymmetricEigenDecomposition) -> f64 {
        let n = matrix.rows();
        let scale = matrix.values().iter().map(|v| v * v).sum::<f64>().sqrt();
        let mut worst: f64 = 0.0;
        for (index, lambda) in result.eigenvalues().iter().enumerate() {
            let vector = &result.eigenvectors().values()[index * n..(index + 1) * n];
            let mut residual = 0.0;
            for row in 0..n {
                let mut product = Complex::<f64>::zero();
                for (inner, entry) in vector.iter().enumerate() {
                    let weight = matrix.values()[inner * n + row];
                    product += Complex::new(weight * entry.real(), weight * entry.imaginary());
                }
                residual += (product - *lambda * vector[row]).norm_squared();
            }
            worst = worst.max(residual.sqrt() / scale);
        }
        worst
    }

    #[test]
    fn faer_residuals_are_large_for_degenerate_oscillators_and_refined_to_the_contract() {
        let cases = [
            oscillators(4, 0),
            oscillators(3, 1),
            oscillators(3, 3),
            triple_state_space(100),
        ];
        for matrix in cases {
            let raw = worst_ratio(&matrix, &solve_without_refinement(&matrix).unwrap());
            let refined = worst_ratio(&matrix, &solve_nonsymmetric_f64(&matrix).unwrap());
            assert!(raw > 1e-10, "raw {raw:e}");
            assert!(refined <= 1e-12, "refined {refined:e}");
        }
    }

    #[test]
    fn output_without_refinement_targets_is_bitwise_unchanged() {
        let mut seed = 3u64;
        for size in [1usize, 2, 7, 60] {
            let values = (0..size * size)
                .map(|_| {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    ((seed >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
                })
                .collect();
            let matrix = DenseMatrix::from_column_major(Shape::new(size, size), values).unwrap();
            let with = solve_nonsymmetric_f64(&matrix).unwrap();
            let without = solve_without_refinement(&matrix).unwrap();
            assert_eq!(with, without);
            for (left, right) in with
                .eigenvectors()
                .values()
                .iter()
                .zip(without.eigenvectors().values())
            {
                assert_eq!(left.real().to_bits(), right.real().to_bits());
                assert_eq!(left.imaginary().to_bits(), right.imaginary().to_bits());
            }
        }
    }
}
