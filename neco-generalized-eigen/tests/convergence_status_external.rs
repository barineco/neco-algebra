use neco_generalized_eigen::ConvergenceStatus;

#[test]
fn external_consumers_observe_validated_convergence_statuses() {
    let converged = ConvergenceStatus::converged(3, 2, 2, 2, 1.0e-8, 2.0e-8).expect("valid status");
    assert!(converged.is_converged());
    assert_eq!(converged.iterations(), 3);
    assert_eq!(converged.requested_modes(), 2);
    assert_eq!(converged.returned_modes(), 2);
    assert_eq!(converged.converged_modes(), 2);
    assert_eq!(converged.absolute_tolerance(), 1.0e-8);
    assert_eq!(converged.relative_tolerance(), 2.0e-8);

    let limited =
        ConvergenceStatus::iteration_limit(4, 3, 2, 1, 1.0e-8, 2.0e-8).expect("valid status");
    assert!(!limited.is_converged());
    assert!(limited.converged_modes() <= limited.returned_modes());
}
