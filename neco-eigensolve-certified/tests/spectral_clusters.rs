mod common;
mod inputs;

use common::{rational, Entry};
use inputs::{
    diagonal, identity_input, identity_input_from_entries, positive_definite_input, solve,
};
use neco_bigint::Dyadic;
use neco_eigensolve_certified::{certify_spectral_clusters, ClusterError, ModeOrdinal};

fn dyadic(value: f64) -> Dyadic {
    Dyadic::from_f64_exact(value).expect("finite value")
}

fn assert_close(actual: f64, expected: f64) {
    let relative = ((actual - expected) / expected).abs();
    assert!(
        relative <= 1.0e-9,
        "expected {expected}, got {actual} (relative error {relative})"
    );
}

#[test]
fn well_separated_diagonal_spectrum_certifies_every_mode_number() {
    let solution = solve(identity_input(2, diagonal(&[2, 3])), 2);
    let clusters = certify_spectral_clusters(&solution).expect("clusters");

    assert_eq!(clusters.clusters().len(), 2);
    assert_eq!(clusters.mode_ordinal(0), Some(ModeOrdinal::Certified(1)));
    assert_eq!(clusters.mode_ordinal(1), Some(ModeOrdinal::Certified(2)));
    assert_eq!(clusters.mode_ordinal(2), None);
    for (cluster, eigenvalue) in clusters.clusters().iter().zip([2.0, 3.0]) {
        assert!(cluster.radius().is_zero());
        assert_eq!(cluster.interval().lower(), &dyadic(eigenvalue));
        assert_eq!(cluster.interval().upper(), &dyadic(eigenvalue));
        assert_eq!(cluster.separation(), Some(&dyadic(1.0)));
        assert_eq!(cluster.condition_bound(), Some(1.0));
        assert_eq!(cluster.subspace_angle_bound(), 0.0);
    }
}

#[test]
fn positive_definite_route_certifies_mode_numbers_with_nonunit_mass() {
    let solution = solve(
        positive_definite_input(2, diagonal(&[8, 27]), diagonal(&[4, 9])),
        2,
    );
    let clusters = certify_spectral_clusters(&solution).expect("clusters");

    assert_eq!(clusters.clusters().len(), 2);
    assert_eq!(clusters.mode_ordinal(0), Some(ModeOrdinal::Certified(1)));
    assert_eq!(clusters.mode_ordinal(1), Some(ModeOrdinal::Certified(2)));
    assert!(clusters.clusters()[0]
        .interval()
        .contains_dyadic(&dyadic(2.0)));
    assert!(clusters.clusters()[1]
        .interval()
        .contains_dyadic(&dyadic(3.0)));
}

#[test]
fn repeated_eigenvalue_forms_one_certified_cluster() {
    let solution = solve(identity_input(3, diagonal(&[2, 2, 5])), 3);
    assert_eq!(solution.eigenspaces()[0].basis().len(), 2);
    let clusters = certify_spectral_clusters(&solution).expect("clusters");

    assert_eq!(clusters.clusters().len(), 2);
    let repeated = &clusters.clusters()[0];
    assert_eq!(*repeated.ordinals(), 1..=2);
    assert_eq!(*repeated.pair_range(), 0..2);
    assert_eq!(repeated.multiplicity(), 2);
    let within = Some(ModeOrdinal::WithinCluster { first: 1, last: 2 });
    assert_eq!(clusters.mode_ordinal(0), within);
    assert_eq!(clusters.mode_ordinal(1), within);
    assert!(repeated.radius().is_zero());
    assert_eq!(repeated.subspace_angle_bound(), 0.0);
    assert_eq!(repeated.separation(), Some(&dyadic(3.0)));
    assert_eq!(clusters.mode_ordinal(2), Some(ModeOrdinal::Certified(3)));
}

#[test]
fn eigenvalues_closer_than_the_projection_error_form_an_undetermined_cluster() {
    let solution = solve(
        identity_input_from_entries(
            3,
            vec![
                Entry::Sqrt2,
                rational(0),
                rational(0),
                rational(0),
                rational(2),
                rational(0),
                rational(0),
                rational(0),
                Entry::Rational(257, 128),
            ],
            4,
        ),
        3,
    );
    let eta_k = solution
        .projection_reference()
        .stiffness()
        .absolute_error_bound()
        .round_to_f64_ties_even()
        .expect("finite value");
    assert!(eta_k > 2.0_f64.powi(-8) && eta_k < 0.2);

    let clusters = certify_spectral_clusters(&solution).expect("clusters");
    assert_eq!(clusters.clusters().len(), 2);
    assert_eq!(clusters.mode_ordinal(0), Some(ModeOrdinal::Certified(1)));
    let within = Some(ModeOrdinal::WithinCluster { first: 2, last: 3 });
    assert_eq!(clusters.mode_ordinal(1), within);
    assert_eq!(clusters.mode_ordinal(2), within);

    let undetermined = &clusters.clusters()[1];
    assert!(undetermined.interval().contains_dyadic(&dyadic(2.0)));
    assert!(undetermined.interval().contains_dyadic(&dyadic(2.0078125)));
    let radius = undetermined
        .radius()
        .round_to_f64_ties_even()
        .expect("finite value");
    assert_close(radius, 2.0_f64.powi(-8) + std::f64::consts::SQRT_2 * eta_k);
}

#[test]
fn singleton_interval_matches_pair_enclosure_without_mass_projection_error() {
    let solution = solve(
        identity_input_from_entries(
            2,
            vec![Entry::Sqrt2, rational(0), rational(0), rational(3)],
            4,
        ),
        2,
    );
    assert!(
        solution
            .projection_reference()
            .stiffness()
            .absolute_error_bound()
            .round_to_f64_ties_even()
            .expect("finite value")
            > 0.0
    );
    assert!(solution
        .projection_reference()
        .mass()
        .absolute_error_bound()
        .is_zero());

    let clusters = certify_spectral_clusters(&solution).expect("clusters");
    assert_eq!(clusters.clusters().len(), 2);
    for cluster in clusters.clusters() {
        let pair = clusters.pair(cluster.pair_range().start).expect("pair");
        assert_close(
            cluster
                .radius()
                .round_to_f64_ties_even()
                .expect("finite value"),
            pair.enclosure_radius()
                .round_to_f64_ties_even()
                .expect("finite value"),
        );
    }
}

#[test]
fn partial_spectrum_is_rejected_with_a_typed_failure() {
    let solution = solve(identity_input(3, diagonal(&[2, 3, 5])), 2);

    assert!(matches!(
        certify_spectral_clusters(&solution),
        Err(ClusterError::IncompleteSpectrum {
            returned: 2,
            dimension: 3
        })
    ));
}

#[test]
fn repeated_eigenspace_across_the_requested_boundary_is_incomplete() {
    let solution = solve(identity_input(3, diagonal(&[2, 2, 5])), 1);
    let returned: usize = solution
        .eigenspaces()
        .iter()
        .map(|space| space.basis().len())
        .sum();
    assert_eq!(returned, 2);

    assert!(matches!(
        certify_spectral_clusters(&solution),
        Err(ClusterError::IncompleteSpectrum {
            returned: 2,
            dimension: 3
        })
    ));
}

#[test]
fn cluster_intervals_contain_the_analytic_exact_spectrum() {
    let solution = solve(
        positive_definite_input(2, vec![(5, 1), (2, 1), (2, 1), (5, 1)], diagonal(&[1, 4])),
        2,
    );
    let clusters = certify_spectral_clusters(&solution).expect("clusters");

    assert_eq!(clusters.mode_ordinal(0), Some(ModeOrdinal::Certified(1)));
    assert_eq!(clusters.mode_ordinal(1), Some(ModeOrdinal::Certified(2)));
    assert!(clusters.clusters()[0]
        .interval()
        .contains_dyadic(&dyadic(1.0)));
    assert!(clusters.clusters()[1]
        .interval()
        .contains_dyadic(&dyadic(5.25)));
}

#[test]
fn separation_condition_and_angle_follow_the_certified_interval_gap() {
    let solution = solve(
        identity_input_from_entries(
            3,
            vec![
                Entry::Sqrt2,
                rational(0),
                rational(0),
                rational(0),
                rational(3),
                rational(0),
                rational(0),
                rational(0),
                rational(10),
            ],
            4,
        ),
        3,
    );
    let eta = solution
        .projection_reference()
        .stiffness()
        .absolute_error_bound()
        .round_to_f64_ties_even()
        .expect("finite value");
    assert!(eta > 0.0 && eta < 0.2);
    let mus: Vec<f64> = solution
        .eigenspaces()
        .iter()
        .map(|space| space.eigenvalue())
        .collect();
    assert_close(mus[0], std::f64::consts::SQRT_2);

    let clusters = certify_spectral_clusters(&solution).expect("clusters");
    assert_eq!(clusters.clusters().len(), 3);

    let gap_below = mus[1] - mus[0] - 2.0 * eta;
    let gap_above = mus[2] - mus[1] - 2.0 * eta;
    let separations = [gap_below, gap_below.min(gap_above), gap_above];
    let angle_distances = [
        mus[1] - mus[0] - eta,
        (mus[1] - mus[0] - eta).min(mus[2] - mus[1] - eta),
        mus[2] - mus[1] - eta,
    ];
    for ((cluster, separation), distance) in clusters
        .clusters()
        .iter()
        .zip(separations)
        .zip(angle_distances)
    {
        assert_close(
            cluster
                .separation()
                .expect("separation")
                .round_to_f64_ties_even()
                .expect("finite value"),
            separation,
        );
        assert_close(
            cluster.condition_bound().expect("condition bound"),
            1.0 / separation,
        );
        assert_close(cluster.subspace_angle_bound(), eta / distance);
    }
}
