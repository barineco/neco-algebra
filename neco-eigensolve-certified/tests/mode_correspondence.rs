mod common;
mod inputs;

use common::{certified_pair_with_precision, rational, Entry};
use inputs::{
    diagonal, identity_input, identity_input_from_entries, positive_definite_input, solve,
};
use neco_eigensolve_certified::{
    certify_spectral_clusters, correspond_to_reference, CertifiedEigenSolution, ClusterError,
};
use neco_linear_exact::{CertifiedGeneralizedEigenInput, MassNormalizationReference};

fn positive_definite_input_from_entries(
    dimension: usize,
    stiffness: Vec<Entry>,
    mass: Vec<Entry>,
    bits: u32,
) -> CertifiedGeneralizedEigenInput {
    let (stiffness, mass) = certified_pair_with_precision(dimension, stiffness, mass, bits);
    CertifiedGeneralizedEigenInput::new(
        stiffness,
        MassNormalizationReference::positive_definite_cholesky(mass),
    )
    .expect("input")
}

fn assert_close(actual: f64, expected: f64) {
    let relative = ((actual - expected) / expected).abs();
    assert!(
        relative <= 1.0e-9,
        "expected {expected}, got {actual} (relative error {relative})"
    );
}

fn eigenvectors(solution: &CertifiedEigenSolution) -> Vec<Vec<f64>> {
    solution
        .eigenspaces()
        .iter()
        .flat_map(|space| space.basis())
        .map(|pair| pair.eigenvector().values().to_vec())
        .collect()
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

#[test]
fn correspondence_of_identical_well_separated_evaluations_is_one_to_one() {
    let reference_solution = solve(identity_input(2, diagonal(&[2, 3])), 2);
    let candidate = solve(identity_input(2, diagonal(&[2, 3])), 2);
    let reference = certify_spectral_clusters(&reference_solution).expect("reference clusters");

    let correspondence = correspond_to_reference(&reference, &candidate).expect("correspondence");

    let blocks = correspondence.blocks();
    assert_eq!(blocks.len(), 2);
    assert_eq!(*blocks[0].ordinals(), 1..=1);
    assert_eq!(*blocks[1].ordinals(), 2..=2);
    assert_eq!(blocks[0].distance_bound(), 0.0);
    assert_eq!(blocks[1].distance_bound(), 0.0);
    assert_eq!(*blocks[0].reference_clusters(), 0..1);
    assert_eq!(*blocks[0].candidate_clusters(), 0..1);
    assert_eq!(*blocks[1].reference_clusters(), 1..2);
    assert_eq!(*blocks[1].candidate_clusters(), 1..2);
}

#[test]
fn correspondence_bound_encloses_the_true_subspace_angle_of_a_perturbed_evaluation() {
    let reference_solution = solve(identity_input(2, diagonal(&[1, 3])), 2);
    let candidate = solve(identity_input(2, vec![(1, 1), (1, 4), (1, 4), (3, 1)]), 2);
    let reference = certify_spectral_clusters(&reference_solution).expect("reference clusters");

    let correspondence = correspond_to_reference(&reference, &candidate).expect("correspondence");

    let true_sine = (0.5 * (0.25_f64 / 1.0).atan()).sin();
    let blocks = correspondence.blocks();
    assert_eq!(blocks.len(), 2);
    for block in blocks {
        assert!(block.distance_bound() >= true_sine);
        assert!(block.distance_bound() <= 0.32019411);
    }
}

#[test]
fn swapped_evaluation_yields_an_undetermined_block_instead_of_a_wrong_mode_mapping() {
    let reference_solution = solve(identity_input(2, diagonal(&[1, 2])), 2);
    let candidate = solve(identity_input(2, diagonal(&[2, 1])), 2);
    assert_eq!(
        dot(
            &eigenvectors(&reference_solution)[0],
            &eigenvectors(&candidate)[0]
        ),
        0.0
    );
    let reference = certify_spectral_clusters(&reference_solution).expect("reference clusters");

    let correspondence = correspond_to_reference(&reference, &candidate).expect("correspondence");

    let blocks = correspondence.blocks();
    assert_eq!(blocks.len(), 1);
    assert_eq!(*blocks[0].ordinals(), 1..=2);
    assert_eq!(correspondence.block_of(1), correspondence.block_of(2));
    assert!(correspondence.block_of(1).is_some());
    assert_eq!(blocks[0].distance_bound(), 0.0);
    assert_eq!(*blocks[0].reference_clusters(), 0..2);
    assert_eq!(*blocks[0].candidate_clusters(), 0..1);
    assert!(!blocks.iter().any(|block| *block.ordinals() == (1..=1)));
}

#[test]
fn rotated_basis_of_a_repeated_eigenspace_corresponds_by_subspace() {
    let reference_solution = solve(identity_input(3, diagonal(&[2, 2, 5])), 3);
    let candidate = solve(
        identity_input(
            3,
            vec![
                (2, 1),
                (1, 1048576),
                (0, 1),
                (1, 1048576),
                (2, 1),
                (0, 1),
                (0, 1),
                (0, 1),
                (5, 1),
            ],
        ),
        3,
    );
    let reference_vectors = eigenvectors(&reference_solution);
    let candidate_vectors = eigenvectors(&candidate);
    for (reference_vector, candidate_vector) in
        reference_vectors.iter().zip(&candidate_vectors).take(2)
    {
        assert!(dot(reference_vector, candidate_vector).abs() < 0.9);
    }
    let reference = certify_spectral_clusters(&reference_solution).expect("reference clusters");

    let correspondence = correspond_to_reference(&reference, &candidate).expect("correspondence");

    let blocks = correspondence.blocks();
    assert_eq!(blocks.len(), 2);
    assert_eq!(*blocks[0].ordinals(), 1..=2);
    assert_eq!(*blocks[1].ordinals(), 3..=3);
    assert_eq!(*blocks[0].reference_clusters(), 0..1);
    assert_eq!(*blocks[0].candidate_clusters(), 0..1);
    assert_eq!(*blocks[1].reference_clusters(), 1..2);
    assert_eq!(*blocks[1].candidate_clusters(), 1..2);

    let mut difference_squares = 0.0;
    for row in 0..3 {
        for column in 0..3 {
            let projection = |vectors: &[Vec<f64>]| -> f64 {
                vectors[..2]
                    .iter()
                    .map(|vector| vector[row] * vector[column])
                    .sum()
            };
            let entry = projection(&reference_vectors) - projection(&candidate_vectors);
            difference_squares += entry * entry;
        }
    }
    let frobenius = difference_squares.sqrt();
    assert!(blocks[0].distance_bound() <= 1.3e-6);
    assert!(blocks[0].distance_bound() >= frobenius);
}

#[test]
fn correspondence_adds_the_reference_projection_error_to_the_perturbation() {
    let entries = || vec![Entry::Sqrt2, rational(0), rational(0), rational(3)];
    let reference_solution = solve(identity_input_from_entries(2, entries(), 4), 2);
    let candidate = solve(identity_input_from_entries(2, entries(), 4), 2);
    let eta_k = reference_solution
        .projection_reference()
        .stiffness()
        .absolute_error_bound()
        .round_to_f64_ties_even()
        .expect("finite value");
    assert!(eta_k > 0.0);
    let reference = certify_spectral_clusters(&reference_solution).expect("reference clusters");

    let correspondence = correspond_to_reference(&reference, &candidate).expect("correspondence");

    let gap = 3.0 - std::f64::consts::SQRT_2 - eta_k;
    let blocks = correspondence.blocks();
    assert_eq!(blocks.len(), 2);
    for block in blocks {
        assert_close(block.distance_bound(), 2.0 * eta_k / gap);
    }
}

#[test]
fn correspondence_rejects_dimension_mismatch() {
    let reference_solution = solve(identity_input(2, diagonal(&[2, 3])), 2);
    let candidate = solve(identity_input(3, diagonal(&[2, 3, 5])), 3);
    let reference = certify_spectral_clusters(&reference_solution).expect("reference clusters");

    assert!(matches!(
        correspond_to_reference(&reference, &candidate),
        Err(ClusterError::DimensionMismatch {
            reference: 2,
            candidate: 3
        })
    ));
}

#[test]
fn correspondence_rejects_mass_perturbation_exceeding_the_mass_lower_bound() {
    let reference_solution = solve(
        positive_definite_input(2, diagonal(&[2, 4]), diagonal(&[2, 2])),
        2,
    );
    let candidate = solve(
        positive_definite_input(2, diagonal(&[1, 2]), diagonal(&[1, 1])),
        2,
    );
    for solution in [&reference_solution, &candidate] {
        let eigenvalues: Vec<f64> = solution
            .eigenspaces()
            .iter()
            .map(|space| space.eigenvalue())
            .collect();
        assert_eq!(eigenvalues.len(), 2);
        assert!((eigenvalues[0] - 1.0).abs() <= 1.0e-12);
        assert!((eigenvalues[1] - 2.0).abs() <= 1.0e-12);
    }
    let reference = certify_spectral_clusters(&reference_solution).expect("reference clusters");

    assert!(matches!(
        correspond_to_reference(&reference, &candidate),
        Err(ClusterError::MassPerturbationTooLarge)
    ));
}

#[test]
fn correspondence_rejects_an_incomplete_candidate() {
    let reference_solution = solve(identity_input(3, diagonal(&[2, 3, 5])), 3);
    let candidate = solve(identity_input(3, diagonal(&[2, 3, 5])), 2);
    let reference = certify_spectral_clusters(&reference_solution).expect("reference clusters");

    assert!(matches!(
        correspond_to_reference(&reference, &candidate),
        Err(ClusterError::IncompleteSpectrum {
            returned: 2,
            dimension: 3
        })
    ));
}

#[test]
fn correspondence_adds_the_reference_mass_projection_error_to_the_perturbation() {
    let input = || {
        positive_definite_input_from_entries(
            2,
            vec![rational(2), rational(0), rational(0), rational(5)],
            vec![Entry::Sqrt2, rational(0), rational(0), rational(3)],
            4,
        )
    };
    let reference_solution = solve(input(), 2);
    let candidate = solve(input(), 2);
    let eta_m = reference_solution
        .projection_reference()
        .mass()
        .absolute_error_bound()
        .round_to_f64_ties_even()
        .expect("finite value");
    let sigma = reference_solution
        .mass_lower_bound()
        .round_to_f64_ties_even()
        .expect("finite value");
    assert!(eta_m > 0.0 && eta_m < sigma);

    let pairs: Vec<_> = candidate
        .eigenspaces()
        .iter()
        .flat_map(|space| space.basis())
        .collect();
    let mus: Vec<f64> = pairs.iter().map(|pair| pair.eigenvalue()).collect();
    let mass_diagonal = [std::f64::consts::SQRT_2, 3.0];
    let radii: Vec<f64> = pairs
        .iter()
        .map(|pair| {
            let x = pair.eigenvector().values();
            let q: f64 = x.iter().map(|value| value * value).sum();
            let nu: f64 = x
                .iter()
                .zip(mass_diagonal)
                .map(|(value, mass)| value * value * mass)
                .sum();
            let d = pair
                .computational_bound()
                .round_to_f64_ties_even()
                .expect("finite value");
            let computational = (sigma * d * d * nu / (sigma - eta_m)).sqrt();
            let perturbation = pair.eigenvalue().abs() * eta_m * q.sqrt() / (sigma - eta_m).sqrt();
            (computational + perturbation) / (nu - eta_m * q).sqrt()
        })
        .collect();
    let gap = mus[1] - mus[0];
    assert!(radii[0] + radii[1] < gap);
    let reference = certify_spectral_clusters(&reference_solution).expect("reference clusters");

    let correspondence = correspond_to_reference(&reference, &candidate).expect("correspondence");

    let blocks = correspondence.blocks();
    assert_eq!(blocks.len(), 2);
    assert_close(
        blocks[0].distance_bound(),
        2.0 * radii[0] / (gap - radii[1]),
    );
    assert_close(
        blocks[1].distance_bound(),
        2.0 * radii[1] / (gap - radii[0]),
    );
}
