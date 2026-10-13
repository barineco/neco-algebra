use neco_eigensolve::{
    solve_request_symmetric_f64, solve_symmetric_f64, EigensolveConfig, EigensolveRequest,
};
use neco_expr::{
    allocate_exact_numeric, assemble_exact_computation_product, decide_exact_properties,
    normalize_exact_expressions, read_exact_numeric_inputs, resolve_certified_f64, AbsoluteBits,
    AtomId, AtomStore, ConsumerId, ExactAllocationInput, ExactDecisionRequest,
    ExactExpressionRequirement, ExactOperation, ExactValue, ExprGraph, ExprId, ExprNode,
    NecoImplementationSource, NecoObservedCapability, OwnerExpressionSet, OwnerId,
};
use neco_formsum::FormSum;
use neco_generalized_eigen::GeneralizedEigenProblem;
use neco_linear_dense::DenseMatrix;
use neco_linear_exact::{
    CertifiedGeneralizedEigenInput, CertifiedMatrixInput, MassNormalizationMethod,
    MassNormalizationReference,
};
use neco_linear_types::Shape;

const OPERATIONS: [ExactOperation; 8] = [
    ExactOperation::NormalizeMonomial,
    ExactOperation::NormalizeFormSum,
    ExactOperation::NormalizeAlgebraic,
    ExactOperation::DecideZero,
    ExactOperation::DecideEquality,
    ExactOperation::DecideSign,
    ExactOperation::DecideDegeneracy,
    ExactOperation::ResolveCertifiedF64,
];

fn integer_form_sum(value: u32) -> FormSum {
    let mut sum = FormSum::zero();
    for _ in 0..value {
        sum = sum.add(&FormSum::one().expect("one")).expect("sum");
    }
    sum
}

fn product() -> neco_expr::ExactComputationProduct {
    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    for (index, value) in [2, 0, 0, 3, 1, 0, 0, 1].into_iter().enumerate() {
        graph
            .push(ExprNode::Atom(AtomId::new(index as u32)))
            .expect("expression");
        atoms
            .insert(
                AtomId::new(index as u32),
                ExactValue::FormSum(integer_form_sum(value)),
            )
            .expect("atom");
    }
    let stiffness = ExactExpressionRequirement::new(
        ConsumerId::new(11),
        &[
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
        ],
        ExactDecisionRequest::Zero(ExprId::new(0)),
        AbsoluteBits::new(30),
    )
    .expect("stiffness requirement");
    let mass = ExactExpressionRequirement::new(
        ConsumerId::new(12),
        &[
            ExprId::new(4),
            ExprId::new(5),
            ExprId::new(6),
            ExprId::new(7),
        ],
        ExactDecisionRequest::Zero(ExprId::new(4)),
        AbsoluteBits::new(30),
    )
    .expect("mass requirement");
    let first_owner = OwnerExpressionSet::new(OwnerId::new(1), graph, atoms, vec![stiffness, mass])
        .expect("first_owner");
    let second_owner =
        OwnerExpressionSet::new(OwnerId::new(2), ExprGraph::new(), AtomStore::new(), vec![])
            .expect("second_owner");
    let input = ExactAllocationInput::new(
        vec![first_owner, second_owner],
        NecoObservedCapability::current(&OPERATIONS).expect("capability"),
        NecoImplementationSource::current(),
    );
    assemble_exact_computation_product(
        resolve_certified_f64(
            decide_exact_properties(
                normalize_exact_expressions(
                    allocate_exact_numeric(read_exact_numeric_inputs(input).expect("read"))
                        .expect("allocate"),
                )
                .expect("normalize"),
            )
            .expect("decide"),
        )
        .expect("resolve"),
    )
    .expect("assemble")
}

fn certified_input_pair() -> (CertifiedMatrixInput, CertifiedMatrixInput) {
    let product = product();
    let stiffness_projection = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(11))
        .expect("stiffness projection");
    let mass_projection = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(12))
        .expect("mass projection");
    let stiffness = CertifiedMatrixInput::from_product_projection(
        &product,
        stiffness_projection,
        DenseMatrix::from_row_major(Shape::new(2, 2), vec![2.0, 0.0, 0.0, 3.0]).expect("matrix"),
        vec![
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
        ],
        OwnerId::new(1),
        ConsumerId::new(11),
    )
    .expect("stiffness input");
    let mass = CertifiedMatrixInput::from_product_projection(
        &product,
        mass_projection,
        DenseMatrix::from_row_major(Shape::new(2, 2), vec![1.0, 0.0, 0.0, 1.0]).expect("matrix"),
        vec![
            ExprId::new(4),
            ExprId::new(5),
            ExprId::new(6),
            ExprId::new(7),
        ],
        OwnerId::new(1),
        ConsumerId::new(12),
    )
    .expect("mass input");
    (stiffness, mass)
}

fn config() -> EigensolveConfig {
    EigensolveConfig::new(2, 1.0e-12, 1.0e-12, 64).expect("config")
}

#[test]
fn identity_mass_request_preserves_matrix_input_reference() {
    let (stiffness, mass) = certified_input_pair();
    let problem =
        GeneralizedEigenProblem::from_dense(stiffness.matrix().clone(), mass.matrix().clone())
            .expect("problem");
    let reference = CertifiedGeneralizedEigenInput::new(
        stiffness,
        MassNormalizationReference::new(mass, MassNormalizationMethod::Identity).unwrap(),
    )
    .expect("reference");
    let result = solve_request_symmetric_f64(EigensolveRequest::new(problem, config(), reference))
        .expect("result");
    assert!(result.convergence().is_converged());
    assert_eq!(result.eigenspaces()[0].eigenvalue(), 2.0);
    assert_eq!(result.eigenspaces()[1].eigenvalue(), 3.0);
    let stiffness_entry = result
        .projection_reference()
        .stiffness()
        .entry(0, 0)
        .expect("stiffness entry");
    let mass_entry = result
        .projection_reference()
        .mass()
        .matrix()
        .entry(0, 0)
        .expect("mass entry");
    assert_eq!(stiffness_entry.expression(), ExprId::new(0));
    assert_eq!(stiffness_entry.value(), 2.0);
    assert_eq!(mass_entry.expression(), ExprId::new(4));
    assert_eq!(mass_entry.value(), 1.0);
    assert_ne!(stiffness_entry.expression(), mass_entry.expression());
}

#[test]
fn eigensolve_result_preserves_matrix_input_reference() {
    let (stiffness, mass) = certified_input_pair();
    let problem =
        GeneralizedEigenProblem::from_dense(stiffness.matrix().clone(), mass.matrix().clone())
            .expect("problem");
    let reference =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("reference");
    let result = solve_request_symmetric_f64(EigensolveRequest::new(problem, config(), reference))
        .expect("result");
    let mass_entry = result
        .projection_reference()
        .mass()
        .matrix()
        .entry(0, 0)
        .expect("mass entry");
    assert_eq!(mass_entry.consumer_id(), ConsumerId::new(12));
    assert_eq!(mass_entry.expression(), ExprId::new(4));
    assert_eq!(mass_entry.value(), 1.0);
}

#[test]
fn residual_trace_reaches_source_expression() {
    let (stiffness, mass) = certified_input_pair();
    let expected_expression = stiffness.entry(0, 1).unwrap().expression();
    let expected_bound = neco_bigint::Dyadic::from_f64_exact(0.0).expect("zero");
    let problem =
        GeneralizedEigenProblem::from_dense(stiffness.matrix().clone(), mass.matrix().clone())
            .expect("problem");
    let reference =
        CertifiedGeneralizedEigenInput::new(stiffness, MassNormalizationReference::identity(mass))
            .expect("reference");
    let result = solve_request_symmetric_f64(EigensolveRequest::new(problem, config(), reference))
        .expect("result");
    let residual = result
        .eigenspaces()
        .iter()
        .flat_map(|space| space.basis())
        .next()
        .expect("eigenpair")
        .residual();
    assert!(residual.absolute().is_finite());
    assert!(residual.absolute() <= config().absolute_tolerance());
    assert!(residual.relative() <= config().relative_tolerance());
    let entry = result
        .projection_reference()
        .stiffness()
        .entry(0, 1)
        .expect("entry");
    assert_eq!(entry.expression(), expected_expression);
    assert_eq!(entry.consumer_id(), ConsumerId::new(11));
    assert_eq!(entry.absolute_error_bound(), &expected_bound);
    assert_eq!(entry.source(), NecoImplementationSource::current());
}

#[test]
fn low_level_solver_does_not_claim_matrix_provenance() {
    let matrix =
        DenseMatrix::from_row_major(Shape::new(2, 2), vec![1.0, 0.0, 0.0, 1.0]).expect("matrix");
    let problem = GeneralizedEigenProblem::from_dense(matrix.clone(), matrix).expect("problem");
    let result = solve_symmetric_f64(&problem, config()).expect("result");
    assert_eq!(result.projection_reference(), &());
}
