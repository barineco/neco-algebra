use neco_eigensolve_certified::{
    solve_certified_generalized_eigen, CertifiedEigenError, SolverFailure, UndeterminableReason,
};
use neco_expr::{
    allocate_exact_numeric, assemble_exact_computation_product, decide_exact_properties,
    normalize_exact_expressions, read_exact_numeric_inputs, resolve_certified_f64, AbsoluteBits,
    AtomId, AtomStore, ConsumerId, ExactAllocationInput, ExactDecisionRequest,
    ExactExpressionRequirement, ExprGraph, ExprId, ExprNode, NecoImplementationSource,
    NecoObservedCapability, OwnerExpressionSet, OwnerId,
};
use neco_linear_dense::DenseMatrix;
use neco_linear_exact::{
    CertifiedGeneralizedEigenInput, CertifiedMatrixInput, MassNormalizationReference,
};
use neco_linear_types::Shape;

mod common;

use common::{
    certified_pair, certified_pair_with_precision, config, rational, rational_value, Entry,
    OPERATIONS,
};

fn identity_pair() -> (CertifiedMatrixInput, CertifiedMatrixInput) {
    certified_pair(
        2,
        vec![(2, 1), (0, 1), (0, 1), (3, 1)],
        vec![(1, 1), (0, 1), (0, 1), (1, 1)],
    )
}

fn positive_definite_pair() -> (CertifiedMatrixInput, CertifiedMatrixInput) {
    certified_pair(
        2,
        vec![(8, 1), (0, 1), (0, 1), (27, 1)],
        vec![(4, 1), (0, 1), (0, 1), (9, 1)],
    )
}

#[test]
fn identity_route_computes_certified_eigenvalue_residual_and_frequency() {
    let (stiffness, mass) = identity_pair();
    let input =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("input");
    let solution =
        solve_certified_generalized_eigen(input, config(2, 64)).expect("certified solution");

    let eigenvalues: Vec<f64> = solution
        .eigenspaces()
        .iter()
        .map(|space| space.eigenvalue())
        .collect();
    assert_eq!(eigenvalues, vec![2.0, 3.0]);

    for space in solution.eigenspaces() {
        for pair in space.basis() {
            assert_eq!(pair.residual().absolute(), 0.0);
            assert_eq!(
                pair.computational_bound().round_to_f64_ties_even().unwrap(),
                0.0
            );
            assert_eq!(
                pair.projection_bound().round_to_f64_ties_even().unwrap(),
                0.0
            );
            assert_eq!(
                pair.enclosure_radius().round_to_f64_ties_even().unwrap(),
                0.0
            );
            assert!(pair.frequency().lower_bound() <= pair.frequency().value());
            assert!(pair.frequency().value() <= pair.frequency().upper_bound());
        }
    }
    let expected_frequency = [0.22507907903927654, 0.27566444771089604];
    for (space, expected) in solution.eigenspaces().iter().zip(expected_frequency) {
        let actual = space.basis()[0].frequency().value();
        assert!((actual - expected).abs() <= 1.0e-9);
    }
}

#[test]
fn positive_definite_route_computes_certified_eigenvalue_and_lineage() {
    let (stiffness, mass) = positive_definite_pair();
    let input = CertifiedGeneralizedEigenInput::new(
        stiffness,
        MassNormalizationReference::positive_definite_cholesky(mass),
    )
    .expect("input");
    let solution =
        solve_certified_generalized_eigen(input, config(2, 64)).expect("certified solution");

    let eigenvalues: Vec<f64> = solution
        .eigenspaces()
        .iter()
        .map(|space| space.eigenvalue())
        .collect();
    assert_eq!(eigenvalues, vec![2.0, 3.0]);

    let reference = solution.projection_reference();
    let mass_entry = reference.mass().matrix().entry(0, 0).expect("mass entry");
    assert_eq!(mass_entry.value(), 4.0);
    assert_eq!(mass_entry.expression(), ExprId::new(4));

    let stiffness_entry = reference.stiffness().entry(1, 1).expect("stiffness entry");
    assert_eq!(stiffness_entry.value(), 27.0);
    assert_eq!(stiffness_entry.expression(), ExprId::new(3));
    assert_eq!(
        reference.stiffness().absolute_error_bound(),
        reference.mass().absolute_error_bound()
    );
    assert_eq!(
        reference.product_identity(),
        reference.stiffness().product_identity()
    );

    for space in solution.eigenspaces() {
        for pair in space.basis() {
            assert_eq!(
                pair.computational_bound().round_to_f64_ties_even().unwrap(),
                0.0
            );
            assert_eq!(
                pair.projection_bound().round_to_f64_ties_even().unwrap(),
                0.0
            );
        }
    }
}

#[test]
fn mass_lower_bound_fails_for_non_diagonally_dominant_positive_definite_mass() {
    let (stiffness, mass) = certified_pair(
        2,
        vec![(2, 1), (0, 1), (0, 1), (3, 1)],
        vec![(1, 1), (3, 2), (3, 2), (4, 1)],
    );
    let input = CertifiedGeneralizedEigenInput::new(
        stiffness,
        MassNormalizationReference::positive_definite_cholesky(mass),
    )
    .expect("input");

    match solve_certified_generalized_eigen(input, config(2, 64)) {
        Err(CertifiedEigenError::Undeterminable { failures }) => {
            assert_eq!(failures.len(), 2);
            for failure in failures {
                assert_eq!(
                    failure.reason(),
                    UndeterminableReason::MassLowerBoundUnavailable
                );
                assert_eq!(failure.enclosure_radius(), None);
            }
        }
        other => panic!("expected Undeterminable, got {other:?}"),
    }
}

#[test]
fn requested_modes_exceeding_dimension_is_rejected() {
    let (stiffness, mass) = identity_pair();
    let input =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("input");

    assert!(matches!(
        solve_certified_generalized_eigen(input, config(5, 64)),
        Err(CertifiedEigenError::RequestedModesExceedDimension {
            requested: 5,
            dimension: 2
        })
    ));
}

#[test]
fn repeated_eigenvalue_spanning_the_requested_boundary_is_not_a_dimension_error() {
    let (stiffness, mass) = certified_pair(
        3,
        vec![
            (2, 1),
            (0, 1),
            (0, 1),
            (0, 1),
            (2, 1),
            (0, 1),
            (0, 1),
            (0, 1),
            (5, 1),
        ],
        vec![
            (1, 1),
            (0, 1),
            (0, 1),
            (0, 1),
            (1, 1),
            (0, 1),
            (0, 1),
            (0, 1),
            (1, 1),
        ],
    );
    let input =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("input");
    let solution =
        solve_certified_generalized_eigen(input, config(1, 64)).expect("certified solution");

    assert_eq!(solution.eigenspaces().len(), 1);
    assert_eq!(solution.eigenspaces()[0].eigenvalue(), 2.0);
    assert_eq!(solution.eigenspaces()[0].basis().len(), 2);
    for pair in solution.eigenspaces()[0].basis() {
        assert_eq!(pair.residual().absolute(), 0.0);
    }
}

#[test]
fn residual_not_converged_lists_the_failing_eigenpairs() {
    let (stiffness, mass) = certified_pair(
        3,
        vec![
            (4, 1),
            (1, 1),
            (0, 1),
            (1, 1),
            (4, 1),
            (1, 1),
            (0, 1),
            (1, 1),
            (4, 1),
        ],
        vec![
            (1, 1),
            (0, 1),
            (0, 1),
            (0, 1),
            (1, 1),
            (0, 1),
            (0, 1),
            (0, 1),
            (1, 1),
        ],
    );
    let input =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("input");

    match solve_certified_generalized_eigen(input, config(3, 1)) {
        Err(CertifiedEigenError::ResidualNotConverged {
            convergence,
            non_convergent,
        }) => {
            assert_eq!(convergence.iterations(), 1);
            assert_eq!(convergence.requested_modes(), 3);
            assert!(!non_convergent.is_empty());
            let expected_eigenvalues = [2.999999999999999, 4.0, 4.999999999999998];
            for failure in &non_convergent {
                assert!(failure.residual().absolute() > 1.0e-12);
                assert!(expected_eigenvalues
                    .iter()
                    .any(|expected| (failure.eigenvalue() - expected).abs() <= 1.0e-9));
            }
        }
        other => panic!("expected ResidualNotConverged, got {other:?}"),
    }
}

#[test]
fn identity_route_rejects_mislabeled_non_identity_mass() {
    let (stiffness, mass) = certified_pair(
        2,
        vec![(2, 1), (0, 1), (0, 1), (3, 1)],
        vec![(1, 1), (0, 1), (0, 1), (2, 1)],
    );
    let input =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("input");

    assert!(matches!(
        solve_certified_generalized_eigen(input, config(2, 64)),
        Err(CertifiedEigenError::InvalidMass(
            SolverFailure::IdentityMass(
                neco_eigensolve::EigensolveError::UnsupportedMassMatrix { .. }
            )
        ))
    ));
}

#[test]
fn positive_definite_route_rejects_indefinite_mass() {
    let (stiffness, mass) = certified_pair(
        2,
        vec![(2, 1), (0, 1), (0, 1), (3, 1)],
        vec![(1, 1), (0, 1), (0, 1), (-1, 1)],
    );
    let input = CertifiedGeneralizedEigenInput::new(
        stiffness,
        MassNormalizationReference::positive_definite_cholesky(mass),
    )
    .expect("input");

    assert!(matches!(
        solve_certified_generalized_eigen(input, config(2, 64)),
        Err(CertifiedEigenError::InvalidMass(
            SolverFailure::PositiveDefiniteMass(
                neco_eigensolve_faer::EigensolveFaerError::NonPositiveMassMatrix
            )
        ))
    ));
}

#[test]
fn identity_route_rejects_nonsymmetric_stiffness() {
    let (stiffness, mass) = certified_pair(
        2,
        vec![(1, 1), (2, 1), (3, 1), (4, 1)],
        vec![(1, 1), (0, 1), (0, 1), (1, 1)],
    );
    let input =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("input");

    assert!(matches!(
        solve_certified_generalized_eigen(input, config(2, 64)),
        Err(CertifiedEigenError::InvalidStiffness(
            SolverFailure::IdentityMass(
                neco_eigensolve::EigensolveError::InvalidStiffnessMatrix { .. }
            )
        ))
    ));
}

#[test]
fn non_square_matrices_are_rejected_as_an_invalid_problem_shape() {
    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    for index in 0..4_u32 {
        graph
            .push(ExprNode::Atom(AtomId::new(index)))
            .expect("expression");
        atoms
            .insert(AtomId::new(index), rational_value(1, 1))
            .expect("atom");
    }
    let stiffness_ids = vec![ExprId::new(0), ExprId::new(1)];
    let mass_ids = vec![ExprId::new(2), ExprId::new(3)];
    let stiffness_requirement = ExactExpressionRequirement::new(
        ConsumerId::new(11),
        &stiffness_ids,
        ExactDecisionRequest::Zero(stiffness_ids[0]),
        AbsoluteBits::new(30),
    )
    .expect("stiffness requirement");
    let mass_requirement = ExactExpressionRequirement::new(
        ConsumerId::new(12),
        &mass_ids,
        ExactDecisionRequest::Zero(mass_ids[0]),
        AbsoluteBits::new(30),
    )
    .expect("mass requirement");
    let first_owner = OwnerExpressionSet::new(
        OwnerId::new(1),
        graph,
        atoms,
        vec![stiffness_requirement, mass_requirement],
    )
    .expect("first_owner");
    let second_owner =
        OwnerExpressionSet::new(OwnerId::new(2), ExprGraph::new(), AtomStore::new(), vec![])
            .expect("second_owner");
    let allocation_input = ExactAllocationInput::new(
        vec![first_owner, second_owner],
        NecoObservedCapability::current(&OPERATIONS).expect("capability"),
        NecoImplementationSource::current(),
    );
    let product = assemble_exact_computation_product(
        resolve_certified_f64(
            decide_exact_properties(
                normalize_exact_expressions(
                    allocate_exact_numeric(
                        read_exact_numeric_inputs(allocation_input).expect("read"),
                    )
                    .expect("allocate"),
                )
                .expect("normalize"),
            )
            .expect("decide"),
        )
        .expect("resolve"),
    )
    .expect("assemble");
    let stiffness_projection = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(11))
        .expect("stiffness projection");
    let mass_projection = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(12))
        .expect("mass projection");
    let shape = Shape::new(1, 2);
    let stiffness_matrix =
        DenseMatrix::from_row_major(shape, vec![1.0, 1.0]).expect("stiffness matrix");
    let mass_matrix = DenseMatrix::from_row_major(shape, vec![1.0, 1.0]).expect("mass matrix");
    let stiffness = CertifiedMatrixInput::from_product_projection(
        &product,
        stiffness_projection,
        stiffness_matrix,
        stiffness_ids,
        OwnerId::new(1),
        ConsumerId::new(11),
    )
    .expect("stiffness input");
    let mass = CertifiedMatrixInput::from_product_projection(
        &product,
        mass_projection,
        mass_matrix,
        mass_ids,
        OwnerId::new(1),
        ConsumerId::new(12),
    )
    .expect("mass input");
    let input =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("input accepts matching non-square shapes");

    assert!(matches!(
        solve_certified_generalized_eigen(input, config(1, 64)),
        Err(CertifiedEigenError::InvalidProblemShape(
            neco_generalized_eigen::GeneralizedEigenError::NonSquareMatrix {
                rows: 1,
                columns: 2
            }
        ))
    ));
}

#[test]
fn nonzero_stiffness_projection_error_alone_moves_the_projection_bound() {
    let (stiffness, mass) = certified_pair_with_precision(
        2,
        vec![Entry::Sqrt2, rational(0), rational(0), rational(3)],
        vec![rational(1), rational(0), rational(0), rational(1)],
        4,
    );
    let input =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("input");
    let solution =
        solve_certified_generalized_eigen(input, config(2, 64)).expect("certified solution");

    let stiffness_error = solution
        .projection_reference()
        .stiffness()
        .absolute_error_bound()
        .round_to_f64_ties_even()
        .expect("finite value");
    assert!(stiffness_error > 0.0);
    assert_eq!(
        solution
            .projection_reference()
            .mass()
            .absolute_error_bound()
            .round_to_f64_ties_even()
            .expect("finite value"),
        0.0
    );

    for space in solution.eigenspaces() {
        for pair in space.basis() {
            let projection = pair
                .projection_bound()
                .round_to_f64_ties_even()
                .expect("finite value");
            assert!((projection - stiffness_error).abs() <= 1.0e-9);
            assert!(projection > 0.0);
        }
    }
}

#[test]
fn oversized_mass_projection_error_is_rejected_through_the_public_function() {
    let (stiffness, mass) = certified_pair_with_precision(
        2,
        vec![rational(2), rational(0), rational(0), rational(3)],
        vec![
            Entry::Sqrt2,
            rational(0),
            rational(0),
            Entry::Rational(1, 64),
        ],
        4,
    );
    let input = CertifiedGeneralizedEigenInput::new(
        stiffness,
        MassNormalizationReference::positive_definite_cholesky(mass),
    )
    .expect("input");

    match solve_certified_generalized_eigen(input, config(2, 64)) {
        Err(CertifiedEigenError::Undeterminable { failures }) => {
            assert_eq!(failures.len(), 2);
            for failure in failures {
                assert_eq!(
                    failure.reason(),
                    UndeterminableReason::ProjectionPerturbationTooLarge
                );
                assert_eq!(failure.enclosure_radius(), None);
            }
        }
        other => panic!("expected Undeterminable, got {other:?}"),
    }
}

#[test]
fn negative_eigenvalue_is_undeterminable_with_evidence() {
    let (stiffness, mass) = certified_pair(
        2,
        vec![(-2, 1), (0, 1), (0, 1), (3, 1)],
        vec![(1, 1), (0, 1), (0, 1), (1, 1)],
    );
    let input =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("input");

    match solve_certified_generalized_eigen(input, config(2, 64)) {
        Err(CertifiedEigenError::Undeterminable { failures }) => {
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0].eigenvalue(), -2.0);
            assert_eq!(
                failures[0].reason(),
                UndeterminableReason::NegativeEigenvalue
            );
            assert_eq!(failures[0].enclosure_radius(), Some(0.0));
        }
        other => panic!("expected Undeterminable, got {other:?}"),
    }
}

#[test]
fn enclosure_lower_bound_below_zero_is_undeterminable_with_evidence() {
    let (stiffness, mass) = certified_pair_with_precision(
        2,
        vec![rational(0), rational(0), rational(0), Entry::Sqrt2],
        vec![rational(1), rational(0), rational(0), rational(1)],
        4,
    );
    let input =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("input");

    match solve_certified_generalized_eigen(input, config(2, 64)) {
        Err(CertifiedEigenError::Undeterminable { failures }) => {
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0].eigenvalue(), 0.0);
            let radius = failures[0].enclosure_radius().expect("radius evidence");
            assert!(radius > 0.0);
            match failures[0].reason() {
                UndeterminableReason::EnclosureLowerBoundBelowZero { lower_bound } => {
                    assert!(lower_bound < 0.0);
                }
                other => panic!("expected EnclosureLowerBoundBelowZero, got {other:?}"),
            }
        }
        other => panic!("expected Undeterminable, got {other:?}"),
    }
}

#[test]
fn swapped_stiffness_and_mass_is_rejected_or_produces_different_eigenvalues() {
    let (stiffness, mass) = positive_definite_pair();
    let swapped = CertifiedGeneralizedEigenInput::new(
        mass,
        MassNormalizationReference::positive_definite_cholesky(stiffness),
    )
    .expect("swapped input");
    let solution =
        solve_certified_generalized_eigen(swapped, config(2, 64)).expect("swapped solution");
    let eigenvalues: Vec<f64> = solution
        .eigenspaces()
        .iter()
        .map(|space| space.eigenvalue())
        .collect();
    assert_ne!(eigenvalues, vec![2.0, 3.0]);
    let expected = [1.0 / 3.0, 0.5];
    for (actual, expected) in eigenvalues.iter().zip(expected) {
        assert!((actual - expected).abs() <= 1.0e-9);
    }

    let (stiffness, mass) = identity_pair();
    let swapped_identity =
        CertifiedGeneralizedEigenInput::new(mass, MassNormalizationReference::identity(stiffness))
            .expect("swapped identity input");
    assert!(matches!(
        solve_certified_generalized_eigen(swapped_identity, config(2, 64)),
        Err(CertifiedEigenError::InvalidMass(
            SolverFailure::IdentityMass(
                neco_eigensolve::EigensolveError::UnsupportedMassMatrix { .. }
            )
        ))
    ));
}

#[test]
fn low_level_solver_routes_do_not_claim_certified_evaluation() {
    let problem = neco_generalized_eigen::GeneralizedEigenProblem::from_dense(
        DenseMatrix::from_row_major(Shape::new(2, 2), vec![2.0, 0.0, 0.0, 3.0]).expect("matrix"),
        DenseMatrix::from_row_major(Shape::new(2, 2), vec![1.0, 0.0, 0.0, 1.0]).expect("matrix"),
    )
    .expect("problem");
    let identity_result =
        neco_eigensolve::solve_symmetric_f64(&problem, config(2, 64)).expect("identity result");
    assert_eq!(identity_result.eigenspaces()[0].eigenvalue(), 2.0);

    let faer_result =
        neco_eigensolve_faer::solve_symmetric_f64(&problem, config(2, 64)).expect("faer result");
    assert_eq!(faer_result.eigenspaces()[0].eigenvalue(), 2.0);
}
