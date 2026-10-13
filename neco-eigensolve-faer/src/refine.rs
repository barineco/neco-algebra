mod groups;
mod residual;
mod subspace;

use crate::nonsymmetric::normalize;
use crate::{try_filled, EigensolveFaerError, RefinementFailureKind};
use neco_complex::Complex;
use residual::{
    condition_number, dependent, exceeds, failure, frobenius, measure_selected, replaces, Split,
    ACCEPT, TRIGGER,
};
use subspace::{refine_group, GroupOutcome, Hessenberg};

pub(crate) fn certify_and_refine(
    matrix: &[f64],
    eigenvalues: &[Complex<f64>],
    partner: &[usize],
    vectors: &mut [Complex<f64>],
    rank: &[usize],
    operation: &'static str,
) -> Result<(), EigensolveFaerError> {
    let dimension = eigenvalues.len();
    let frobenius_norm = frobenius(matrix);
    let measured = measure_selected(matrix, frobenius_norm, eigenvalues, partner, vectors)?;

    let mut upper_count = 0;
    for (slot, &column) in measured.columns.iter().enumerate() {
        if partner[column] == column {
            if exceeds(measured.ratios[slot], TRIGGER) {
                return Err(failure(
                    operation,
                    rank[column],
                    measured.ratios[slot],
                    RefinementFailureKind::RealEigenvalue,
                ));
            }
        } else {
            upper_count += 1;
        }
    }
    let mut upper: Vec<usize> = try_filled(upper_count, 0)?;
    let mut before = try_filled(upper_count, 0.0)?;
    let mut upper_lambda = try_filled(upper_count, Complex::<f64>::zero())?;
    let mut flagged = try_filled(upper_count, false)?;
    let mut position = 0;
    let mut any_flagged = false;
    for (slot, &column) in measured.columns.iter().enumerate() {
        if partner[column] != column {
            upper[position] = column;
            before[position] = measured.ratios[slot];
            upper_lambda[position] = eigenvalues[column];
            flagged[position] = exceeds(measured.ratios[slot], TRIGGER);
            any_flagged |= flagged[position];
            position += 1;
        }
    }
    if !any_flagged {
        return Ok(());
    }

    let groups = groups::form(&upper_lambda, &flagged)?;
    for members in &groups {
        if groups::near_real_axis(groups::center(members, &upper_lambda)) {
            return Err(failure(
                operation,
                rank[upper[members[0]]],
                worst_of(members.iter().map(|&member| before[member])),
                RefinementFailureKind::NearRealAxis,
            ));
        }
    }

    let hessenberg = Hessenberg::new(matrix, dimension)?;
    let mut moduli = try_filled(dimension, 0.0)?;
    let floor = f64::EPSILON * frobenius_norm;
    for members in &groups {
        let first_index = rank[upper[members[0]]];
        let worst_before = worst_of(members.iter().map(|&member| before[member]));
        let center = groups::center(members, &upper_lambda);
        let (assigned, ratios) = match refine_group(
            matrix,
            frobenius_norm,
            floor,
            &hessenberg,
            members,
            &upper_lambda,
            center,
        )? {
            GroupOutcome::Refined(assigned, ratios) => (assigned, ratios),
            GroupOutcome::Dependent => {
                return Err(failure(
                    operation,
                    first_index,
                    worst_before,
                    RefinementFailureKind::LinearlyDependent,
                ));
            }
            GroupOutcome::SubproblemFailed => {
                return Err(failure(
                    operation,
                    first_index,
                    worst_before,
                    RefinementFailureKind::SubproblemFailed,
                ));
            }
        };

        let size = members.len();
        let mut chosen = Split::zeros(dimension, size)?;
        let mut final_ratios = try_filled(size, 0.0)?;
        let mut replaced = try_filled(size, false)?;
        for (slot, &member) in members.iter().enumerate() {
            replaced[slot] = replaces(before[member], ratios[slot]);
            final_ratios[slot] = if replaced[slot] {
                ratios[slot]
            } else {
                before[member]
            };
            for row in 0..dimension {
                let at = slot * dimension + row;
                if replaced[slot] {
                    chosen.re[at] = assigned.re[at];
                    chosen.im[at] = assigned.im[at];
                } else {
                    let value = vectors[upper[member] * dimension + row];
                    chosen.re[at] = *value.real();
                    chosen.im[at] = *value.imaginary();
                }
            }
        }
        let worst_final = worst_of(final_ratios.iter().copied());
        match condition_number(&chosen)? {
            None => {
                return Err(failure(
                    operation,
                    first_index,
                    worst_final,
                    RefinementFailureKind::SubproblemFailed,
                ));
            }
            Some(condition) if dependent(condition) => {
                return Err(failure(
                    operation,
                    first_index,
                    worst_final,
                    RefinementFailureKind::LinearlyDependent,
                ));
            }
            Some(_) => {}
        }
        for (slot, &member) in members.iter().enumerate() {
            if exceeds(final_ratios[slot], ACCEPT) {
                return Err(failure(
                    operation,
                    rank[upper[member]],
                    final_ratios[slot],
                    RefinementFailureKind::ResidualExceeded,
                ));
            }
        }
        for (slot, &member) in members.iter().enumerate() {
            if !replaced[slot] {
                continue;
            }
            let column = upper[member];
            let target = &mut vectors[column * dimension..(column + 1) * dimension];
            for (row, value) in target.iter_mut().enumerate() {
                *value = Complex::new(
                    assigned.re[slot * dimension + row],
                    assigned.im[slot * dimension + row],
                );
            }
            normalize(target, &mut moduli)?;
            let mut conjugates = try_filled(dimension, Complex::<f64>::zero())?;
            for (slot, value) in conjugates.iter_mut().zip(target.iter()) {
                *slot = value.conjugate();
            }
            let other = partner[column];
            vectors[other * dimension..(other + 1) * dimension].copy_from_slice(&conjugates);
        }
    }
    Ok(())
}

fn worst_of(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(0.0, |current: f64, value| {
        if value.is_nan() || value > current {
            value
        } else {
            current
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rotation_blocks(first: f64, second: f64) -> Vec<f64> {
        let n = 4;
        let mut a = vec![0.0; n * n];
        a[n] = -first;
        a[1] = first;
        a[2 * n + 3] = second;
        a[3 * n + 2] = -second;
        a
    }

    fn pair(vector: [(f64, f64); 2], offset: usize, columns: &mut [Complex<f64>]) {
        let scale = 0.5f64.sqrt();
        columns[offset] = Complex::new(vector[0].0 * scale, vector[0].1 * scale);
        columns[offset + 1] = Complex::new(vector[1].0 * scale, vector[1].1 * scale);
    }

    fn exact_vectors() -> Vec<Complex<f64>> {
        let mut vectors = vec![Complex::<f64>::zero(); 16];
        pair([(1.0, 0.0), (0.0, -1.0)], 0, &mut vectors[0..4]);
        pair([(1.0, 0.0), (0.0, 1.0)], 0, &mut vectors[4..8]);
        pair([(1.0, 0.0), (0.0, -1.0)], 2, &mut vectors[8..12]);
        pair([(1.0, 0.0), (0.0, 1.0)], 2, &mut vectors[12..16]);
        vectors
    }

    #[test]
    fn exact_eigenpairs_are_left_unchanged() {
        let a = rotation_blocks(1.0, 3.0);
        let lambda = [
            Complex::new(0.0, 1.0),
            Complex::new(0.0, -1.0),
            Complex::new(0.0, 3.0),
            Complex::new(0.0, -3.0),
        ];
        let mut vectors = exact_vectors();
        let before = vectors.clone();
        certify_and_refine(&a, &lambda, &[1, 0, 3, 2], &mut vectors, &[0, 1, 2, 3], "x").unwrap();
        assert_eq!(vectors, before);
    }

    #[test]
    fn real_eigenvalue_with_a_large_residual_is_reported() {
        let a = vec![1.0, 0.0, 0.0, 2.0];
        let lambda = [Complex::new(1.0, 0.0), Complex::new(2.0, 0.0)];
        let scale = 0.5f64.sqrt();
        let mut vectors = vec![
            Complex::new(scale, 0.0),
            Complex::new(scale, 0.0),
            Complex::new(0.0, 0.0),
            Complex::new(1.0, 0.0),
        ];
        let error =
            certify_and_refine(&a, &lambda, &[0, 1], &mut vectors, &[1, 0], "x").unwrap_err();
        match error {
            EigensolveFaerError::EigenvectorRefinement {
                operation,
                index,
                residual,
                kind,
            } => {
                assert_eq!(operation, "x");
                assert_eq!(index, 1);
                assert!(residual > 1e-3);
                assert_eq!(kind, RefinementFailureKind::RealEigenvalue);
            }
            other => panic!("unexpected failure {other:?}"),
        }
    }

    #[test]
    fn a_group_without_a_second_eigenvector_is_linearly_dependent() {
        let a = rotation_blocks(1.0, 3.0);
        let lambda = [
            Complex::new(0.0, 1.0),
            Complex::new(0.0, -1.0),
            Complex::new(0.0, 1.0),
            Complex::new(0.0, -1.0),
        ];
        let mut vectors = exact_vectors();
        let error =
            certify_and_refine(&a, &lambda, &[1, 0, 3, 2], &mut vectors, &[0, 1, 2, 3], "x")
                .unwrap_err();
        match error {
            EigensolveFaerError::EigenvectorRefinement { index, kind, .. } => {
                assert_eq!(index, 0);
                assert_eq!(kind, RefinementFailureKind::LinearlyDependent);
            }
            other => panic!("unexpected failure {other:?}"),
        }
    }

    #[test]
    fn a_defective_group_cannot_be_refined() {
        let n = 4;
        let mut a = rotation_blocks(1.0, 1.0);
        a[2] = 1.0;
        a[n + 3] = 1.0;
        let lambda = [
            Complex::new(0.0, 1.0),
            Complex::new(0.0, -1.0),
            Complex::new(0.0, 1.0),
            Complex::new(0.0, -1.0),
        ];
        let mut vectors = exact_vectors();
        let error =
            certify_and_refine(&a, &lambda, &[1, 0, 3, 2], &mut vectors, &[0, 1, 2, 3], "x")
                .unwrap_err();
        assert!(matches!(
            error,
            EigensolveFaerError::EigenvectorRefinement {
                kind: RefinementFailureKind::LinearlyDependent,
                ..
            }
        ));
    }

    #[test]
    fn a_group_near_the_real_axis_is_reported_without_refinement() {
        let a = vec![1.0, 1e-8, -1e-8, 1.0];
        let lambda = [Complex::new(1.0, 1e-8), Complex::new(1.0, -1e-8)];
        let mut vectors = vec![
            Complex::new(1.0, 0.0),
            Complex::new(0.0, 0.0),
            Complex::new(1.0, 0.0),
            Complex::new(0.0, 0.0),
        ];
        let error =
            certify_and_refine(&a, &lambda, &[1, 0], &mut vectors, &[0, 1], "x").unwrap_err();
        match error {
            EigensolveFaerError::EigenvectorRefinement { index, kind, .. } => {
                assert_eq!(index, 0);
                assert_eq!(kind, RefinementFailureKind::NearRealAxis);
            }
            other => panic!("unexpected failure {other:?}"),
        }
    }
}
