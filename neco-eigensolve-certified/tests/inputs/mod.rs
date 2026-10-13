use neco_eigensolve_certified::{solve_certified_generalized_eigen, CertifiedEigenSolution};
use neco_linear_exact::{CertifiedGeneralizedEigenInput, MassNormalizationReference};

use crate::common::{certified_pair, certified_pair_with_precision, config, rational, Entry};

pub fn diagonal(values: &[i64]) -> Vec<(i64, i64)> {
    let mut entries = Vec::with_capacity(values.len() * values.len());
    for (row, value) in values.iter().enumerate() {
        for column in 0..values.len() {
            entries.push((if row == column { *value } else { 0 }, 1));
        }
    }
    entries
}

pub fn identity_input(
    dimension: usize,
    stiffness: Vec<(i64, i64)>,
) -> CertifiedGeneralizedEigenInput {
    let identity = vec![1; dimension];
    let (stiffness, mass) = certified_pair(dimension, stiffness, diagonal(&identity));
    CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
        .expect("input")
}

pub fn identity_input_from_entries(
    dimension: usize,
    stiffness: Vec<Entry>,
    bits: u32,
) -> CertifiedGeneralizedEigenInput {
    let mut mass = Vec::with_capacity(dimension * dimension);
    for row in 0..dimension {
        for column in 0..dimension {
            mass.push(rational(if row == column { 1 } else { 0 }));
        }
    }
    let (stiffness, mass) = certified_pair_with_precision(dimension, stiffness, mass, bits);
    CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
        .expect("input")
}

pub fn positive_definite_input(
    dimension: usize,
    stiffness: Vec<(i64, i64)>,
    mass: Vec<(i64, i64)>,
) -> CertifiedGeneralizedEigenInput {
    let (stiffness, mass) = certified_pair(dimension, stiffness, mass);
    CertifiedGeneralizedEigenInput::new(
        stiffness,
        MassNormalizationReference::positive_definite_cholesky(mass),
    )
    .expect("input")
}

pub fn solve(
    input: CertifiedGeneralizedEigenInput,
    requested_modes: usize,
) -> CertifiedEigenSolution {
    solve_certified_generalized_eigen(input, config(requested_modes, 64))
        .expect("certified solution")
}
