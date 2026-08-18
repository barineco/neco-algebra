use neco_expr::{
    allocate_exact_numeric, assemble_exact_computation_product, decide_exact_properties,
    normalize_exact_expressions, read_exact_numeric_inputs, resolve_certified_f64, AbsoluteBits,
    AtomId, AtomStore, ConsumerId, ExactAllocationInput, ExactComputationProduct,
    ExactDecisionKind, ExactDecisionRequest, ExactDecisionValue, ExactExpressionRequirement,
    ExactOperation, ExactValue, ExprGraph, ExprNode, MfpCoreProduct, NecoFailure,
    NecoImplementationSource, NecoObservedCapability, NumericalBudgetComponent,
    NumericalErrorBudget, NumericalOwner, UnsupportedFailurePayload, WavesimDddCoreProduct,
};
use neco_monomial::Monomial;

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

fn domain() -> (ExprGraph, AtomStore) {
    let mut graph = ExprGraph::new();
    graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    graph.push(ExprNode::Atom(AtomId::new(1))).unwrap();
    let mut atoms = AtomStore::new();
    atoms
        .insert(AtomId::new(0), ExactValue::Monomial(Monomial::zero()))
        .unwrap();
    atoms
        .insert(AtomId::new(1), ExactValue::Monomial(Monomial::one()))
        .unwrap();
    (graph, atoms)
}

fn requirement(consumer: u32, kind: u32) -> ExactExpressionRequirement {
    let zero = neco_expr::ExprId::new(0);
    let one = neco_expr::ExprId::new(1);
    let decision = match kind % 4 {
        0 => ExactDecisionRequest::Zero(zero),
        1 => ExactDecisionRequest::Equality(one, one),
        2 => ExactDecisionRequest::Sign(one),
        _ => ExactDecisionRequest::Degeneracy(zero),
    };
    let expressions: &[neco_expr::ExprId] = match decision {
        ExactDecisionRequest::Equality(_, _) => &[one],
        ExactDecisionRequest::Zero(_) | ExactDecisionRequest::Degeneracy(_) => &[zero],
        ExactDecisionRequest::Sign(_) => &[one],
    };
    ExactExpressionRequirement::new(
        ConsumerId::new(consumer),
        expressions,
        decision,
        AbsoluteBits::new(30),
    )
    .unwrap()
}

fn budget(
    owner: NumericalOwner,
    consumer: u32,
    component: NumericalBudgetComponent,
) -> NumericalErrorBudget {
    NumericalErrorBudget::new(
        owner,
        ConsumerId::new(consumer),
        component,
        AbsoluteBits::new(12),
    )
}

fn input() -> ExactAllocationInput {
    let (mfp_graph, mfp_atoms) = domain();
    let mfp_requirements = (0..5).map(|id| requirement(id, id)).collect();
    let mfp_budgets = vec![
        budget(
            NumericalOwner::ModalFieldProjection,
            0,
            NumericalBudgetComponent::ModeTruncation,
        ),
        budget(
            NumericalOwner::ModalFieldProjection,
            1,
            NumericalBudgetComponent::ModeShapeEvaluation,
        ),
        budget(
            NumericalOwner::ModalFieldProjection,
            2,
            NumericalBudgetComponent::DampingRt60Conversion,
        ),
        budget(
            NumericalOwner::ModalFieldProjection,
            3,
            NumericalBudgetComponent::ModeSumAccumulation,
        ),
        budget(
            NumericalOwner::ModalFieldProjection,
            4,
            NumericalBudgetComponent::ReceivedSeriesSamplingAccumulation,
        ),
    ];
    let mfp = MfpCoreProduct::new(mfp_graph, mfp_atoms, mfp_requirements, mfp_budgets).unwrap();

    let (wavesim_graph, wavesim_atoms) = domain();
    let wavesim_requirements = (5..12).map(|id| requirement(id, id)).collect();
    let wavesim_budgets = vec![
        budget(
            NumericalOwner::Wavesim,
            5,
            NumericalBudgetComponent::SolverTolerance,
        ),
        budget(
            NumericalOwner::Wavesim,
            6,
            NumericalBudgetComponent::MaximumStepDiscretization,
        ),
        budget(
            NumericalOwner::Wavesim,
            7,
            NumericalBudgetComponent::OdeIntegration,
        ),
        budget(
            NumericalOwner::Wavesim,
            8,
            NumericalBudgetComponent::EnergyBalanceTolerance,
        ),
        budget(
            NumericalOwner::Wavesim,
            9,
            NumericalBudgetComponent::SmoothingRequirement,
        ),
        budget(
            NumericalOwner::Wavesim,
            10,
            NumericalBudgetComponent::DddRegressionEstimation,
        ),
        budget(
            NumericalOwner::Wavesim,
            10,
            NumericalBudgetComponent::SeaRegressionEstimation,
        ),
        budget(
            NumericalOwner::Wavesim,
            11,
            NumericalBudgetComponent::HeldOutComparatorTolerance,
        ),
        budget(
            NumericalOwner::Wavesim,
            11,
            NumericalBudgetComponent::R2AcceptanceMargin,
        ),
    ];
    let wavesim = WavesimDddCoreProduct::new(
        wavesim_graph,
        wavesim_atoms,
        wavesim_requirements,
        wavesim_budgets,
    )
    .unwrap();

    ExactAllocationInput::new(
        mfp,
        wavesim,
        NecoObservedCapability::current(&OPERATIONS).unwrap(),
        NecoImplementationSource::current(),
    )
}

fn product() -> ExactComputationProduct {
    let input = read_exact_numeric_inputs(input()).unwrap();
    let allocated = allocate_exact_numeric(input).unwrap();
    let normalized = normalize_exact_expressions(allocated).unwrap();
    let decided = decide_exact_properties(normalized).unwrap();
    let resolved = resolve_certified_f64(decided).unwrap();
    assemble_exact_computation_product(resolved).unwrap()
}

#[test]
fn one_descent_per_consumer_and_expression_precision_reuse() {
    let product = product();
    let inspection = product.direct_inspection().unwrap();
    assert_eq!(inspection.requirements().len(), 12);
    assert_eq!(inspection.certified_descents().len(), 12);
    assert_eq!(inspection.shared_resolution_count(), 4);
    for requirement in inspection.requirements() {
        let bundle = product
            .certified_descent(requirement.consumer_id())
            .unwrap();
        assert_eq!(bundle.consumer_id(), requirement.consumer_id());
        assert_eq!(bundle.values().len(), requirement.expressions().len());
    }
}

#[test]
fn exact_error_and_owner_budgets_are_disjoint() {
    let product = product();
    let inspection = product.direct_inspection().unwrap();
    let allocation = inspection.allocation();
    assert_eq!(allocation.numerical_error_budgets().len(), 14);
    assert!(allocation
        .numerical_error_budgets()
        .iter()
        .any(|budget| budget.owner() == NumericalOwner::ModalFieldProjection));
    assert!(allocation
        .numerical_error_budgets()
        .iter()
        .any(|budget| budget.owner() == NumericalOwner::Wavesim));
    for bundle in inspection.certified_descents() {
        for value in bundle.values() {
            assert!(value.certified().value().is_finite());
            let _exact_only = value.certified().absolute_error();
        }
    }
}

#[test]
fn typed_decisions_use_normal_forms() {
    let product = product();
    let expected = [
        (0, ExactDecisionKind::Zero, ExactDecisionValue::Zero),
        (1, ExactDecisionKind::Equality, ExactDecisionValue::Equal),
        (2, ExactDecisionKind::Sign, ExactDecisionValue::Positive),
        (
            3,
            ExactDecisionKind::Degeneracy,
            ExactDecisionValue::Degenerate,
        ),
    ];
    for (consumer, kind, value) in expected {
        let witness = product.decision(ConsumerId::new(consumer)).unwrap();
        assert_eq!(witness.kind(), kind);
        assert_eq!(witness.value(), value);
        assert_eq!(
            witness.normal_form_witness().is_zero(),
            matches!(consumer, 0 | 1 | 3)
        );
    }
}

#[test]
fn source_and_required_operation_failures_are_total() {
    let (mfp_graph, mfp_atoms) = domain();
    let mfp = MfpCoreProduct::new(mfp_graph, mfp_atoms, vec![requirement(0, 0)], vec![]).unwrap();
    let (wavesim_graph, wavesim_atoms) = domain();
    let wavesim = WavesimDddCoreProduct::new(wavesim_graph, wavesim_atoms, vec![], vec![]).unwrap();
    let input = ExactAllocationInput::new(
        mfp,
        wavesim,
        NecoObservedCapability::new("wrong", &OPERATIONS).unwrap(),
        NecoImplementationSource::current(),
    );
    assert!(matches!(
        read_exact_numeric_inputs(input),
        Err(NecoFailure::UnsupportedRequiredOperation {
            payload: UnsupportedFailurePayload::SourceIdentity,
            ..
        })
    ));
}

#[test]
fn owner_separates_same_expression_id_for_distinct_monomial_atoms() {
    let mut mfp_graph = ExprGraph::new();
    let expression = mfp_graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let mut mfp_atoms = AtomStore::new();
    mfp_atoms
        .insert(AtomId::new(0), ExactValue::Monomial(Monomial::one()))
        .unwrap();
    let mut wavesim_graph = ExprGraph::new();
    assert_eq!(
        wavesim_graph.push(ExprNode::Atom(AtomId::new(0))).unwrap(),
        expression
    );
    let mut wavesim_atoms = AtomStore::new();
    wavesim_atoms
        .insert(AtomId::new(0), ExactValue::Monomial(Monomial::zero()))
        .unwrap();
    let mfp = MfpCoreProduct::new(
        mfp_graph,
        mfp_atoms,
        vec![ExactExpressionRequirement::new(
            ConsumerId::new(0),
            &[expression],
            ExactDecisionRequest::Sign(expression),
            AbsoluteBits::new(30),
        )
        .unwrap()],
        vec![],
    )
    .unwrap();
    let wavesim = WavesimDddCoreProduct::new(
        wavesim_graph,
        wavesim_atoms,
        vec![ExactExpressionRequirement::new(
            ConsumerId::new(1),
            &[expression],
            ExactDecisionRequest::Zero(expression),
            AbsoluteBits::new(30),
        )
        .unwrap()],
        vec![],
    )
    .unwrap();
    let input = ExactAllocationInput::new(
        mfp,
        wavesim,
        NecoObservedCapability::current(&OPERATIONS).unwrap(),
        NecoImplementationSource::current(),
    );
    let resolved = resolve_certified_f64(
        decide_exact_properties(
            normalize_exact_expressions(
                allocate_exact_numeric(read_exact_numeric_inputs(input).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(resolved.shared_resolution_count(), 2);
    assert_eq!(
        resolved
            .certified_descent(ConsumerId::new(0))
            .unwrap()
            .values()[0]
            .certified()
            .value(),
        1.0
    );
    assert_eq!(
        resolved
            .certified_descent(ConsumerId::new(1))
            .unwrap()
            .values()[0]
            .certified()
            .value(),
        0.0
    );
}

#[test]
fn owner_separates_same_expression_id_for_distinct_algebraic_atoms() {
    let mut mfp_graph = ExprGraph::new();
    let expression = mfp_graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let mut wavesim_graph = ExprGraph::new();
    assert_eq!(
        wavesim_graph.push(ExprNode::Atom(AtomId::new(0))).unwrap(),
        expression
    );
    let mut mfp_atoms = AtomStore::new();
    mfp_atoms
        .insert(
            AtomId::new(0),
            ExactValue::Algebraic(
                neco_algnum::RealAlgebraic::from_integer(neco_bigint::BigInt::try_from(1).unwrap())
                    .unwrap(),
            ),
        )
        .unwrap();
    let mut wavesim_atoms = AtomStore::new();
    wavesim_atoms
        .insert(
            AtomId::new(0),
            ExactValue::Algebraic(
                neco_algnum::RealAlgebraic::from_integer(neco_bigint::BigInt::try_from(2).unwrap())
                    .unwrap(),
            ),
        )
        .unwrap();
    let requirement = |consumer| {
        ExactExpressionRequirement::new(
            ConsumerId::new(consumer),
            &[expression],
            ExactDecisionRequest::Sign(expression),
            AbsoluteBits::new(30),
        )
        .unwrap()
    };
    let mfp = MfpCoreProduct::new(mfp_graph, mfp_atoms, vec![requirement(0)], vec![]).unwrap();
    let wavesim =
        WavesimDddCoreProduct::new(wavesim_graph, wavesim_atoms, vec![requirement(1)], vec![])
            .unwrap();
    let input = ExactAllocationInput::new(
        mfp,
        wavesim,
        NecoObservedCapability::current(&OPERATIONS).unwrap(),
        NecoImplementationSource::current(),
    );
    let resolved = resolve_certified_f64(
        decide_exact_properties(
            normalize_exact_expressions(
                allocate_exact_numeric(read_exact_numeric_inputs(input).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(resolved.shared_resolution_count(), 2);
    let mfp_certified = &resolved
        .certified_descent(ConsumerId::new(0))
        .unwrap()
        .values()[0];
    let wavesim_certified = &resolved
        .certified_descent(ConsumerId::new(1))
        .unwrap()
        .values()[0];
    assert_eq!(mfp_certified.certified().value(), 1.0);
    assert_eq!(wavesim_certified.certified().value(), 2.0);
    assert_ne!(
        mfp_certified.certified().enclosure(),
        wavesim_certified.certified().enclosure()
    );
}

#[test]
fn unsupported_required_operation_is_rejected() {
    let (mfp_graph, mfp_atoms) = domain();
    let mfp = MfpCoreProduct::new(mfp_graph, mfp_atoms, vec![requirement(0, 0)], vec![]).unwrap();
    let (wavesim_graph, wavesim_atoms) = domain();
    let wavesim = WavesimDddCoreProduct::new(wavesim_graph, wavesim_atoms, vec![], vec![]).unwrap();
    let input = ExactAllocationInput::new(
        mfp,
        wavesim,
        NecoObservedCapability::current(&OPERATIONS[..OPERATIONS.len() - 1]).unwrap(),
        NecoImplementationSource::current(),
    );
    let read = read_exact_numeric_inputs(input).unwrap();
    assert!(matches!(
        allocate_exact_numeric(read),
        Err(NecoFailure::UnsupportedRequiredOperation {
            payload: UnsupportedFailurePayload::RequiredOperation("resolve-certified-f64"),
            ..
        })
    ));
}

#[test]
fn resolution_cache_key_includes_precision() {
    let (graph, atoms) = domain();
    let expression = neco_expr::ExprId::new(1);
    let low = ExactExpressionRequirement::new(
        ConsumerId::new(0),
        &[expression],
        ExactDecisionRequest::Sign(expression),
        AbsoluteBits::new(20),
    )
    .unwrap();
    let high = ExactExpressionRequirement::new(
        ConsumerId::new(1),
        &[expression],
        ExactDecisionRequest::Sign(expression),
        AbsoluteBits::new(40),
    )
    .unwrap();
    let mfp = MfpCoreProduct::new(graph, atoms, vec![low, high], vec![]).unwrap();
    let (wavesim_graph, wavesim_atoms) = domain();
    let wavesim = WavesimDddCoreProduct::new(wavesim_graph, wavesim_atoms, vec![], vec![]).unwrap();
    let input = ExactAllocationInput::new(
        mfp,
        wavesim,
        NecoObservedCapability::current(&OPERATIONS).unwrap(),
        NecoImplementationSource::current(),
    );
    let allocated = allocate_exact_numeric(read_exact_numeric_inputs(input).unwrap()).unwrap();
    let normalized = normalize_exact_expressions(allocated).unwrap();
    let decided = decide_exact_properties(normalized).unwrap();
    let resolved = resolve_certified_f64(decided).unwrap();
    assert_eq!(resolved.shared_resolution_count(), 2);
}

#[test]
fn invalid_requirement_constructors_reject_empty_and_missing_operands() {
    let consumer = ConsumerId::new(91);
    let zero = neco_expr::ExprId::new(0);
    let one = neco_expr::ExprId::new(1);
    for result in [
        ExactExpressionRequirement::new(
            consumer,
            &[],
            ExactDecisionRequest::Zero(zero),
            AbsoluteBits::new(20),
        ),
        ExactExpressionRequirement::new(
            consumer,
            &[zero],
            ExactDecisionRequest::Equality(zero, one),
            AbsoluteBits::new(20),
        ),
    ] {
        assert!(matches!(
            result,
            Err(NecoFailure::MissingAssignment { location })
                if location.operation_kind() == neco_expr::FailureOperation::Allocate
                    && location.consumer() == Some(consumer)
        ));
    }
}

#[test]
fn allocation_returns_exact_declared_sets_without_duplicates_or_omissions() {
    use neco_expr::{ExactDecisionAssignment as D, ExactInput as I, NumericalOperation as O};

    const EXACT_INPUTS: [I; 25] = [
        I::GeometryIdentity,
        I::GeometryDimension,
        I::SourceNode,
        I::ReceiverNode,
        I::FrequencyBandEndpoints,
        I::ModeLimit,
        I::SamplingCount,
        I::SamplingRate,
        I::ModeIndexOrdering,
        I::ModeRowCardinality,
        I::ModeRowWidth,
        I::DampingDefinition,
        I::SystemIdentity,
        I::SubsystemIdentity,
        I::StateShape,
        I::StateExtents,
        I::InitialStateIndex,
        I::TimeDomainEndpoints,
        I::CalibrationInterval,
        I::HeldOutInterval,
        I::ConditionIdentity,
        I::CouplingTopology,
        I::CouplingSelector,
        I::ComparatorDirection,
        I::AcceptanceDomainBound,
    ];
    const EXACT_DECISIONS: [D; 19] = [
        D::ProvenanceNonempty,
        D::SourceReceiverIdentityValidity,
        D::ModeSetNonempty,
        D::ModeIdentityEqualityOrdering,
        D::ModeShapeCardinalityAxisEquality,
        D::SamplingDomainValidity,
        D::ZeroDivisionGuard,
        D::Rt60RoundTripEquality,
        D::MfpBranchIdentity,
        D::StateShapeEquality,
        D::CalibrationHeldOutDisjointness,
        D::ConditionSetCompleteness,
        D::SubsystemIdentityEquality,
        D::FiniteNonNegative,
        D::EnergyBalance,
        D::SingularityZeroDenominator,
        D::PredictionIndependenceEquality,
        D::R2LowerBound,
        D::AssemblyFailureSetEmptiness,
    ];
    const NUMERICAL_OPERATIONS: [O; 14] = [
        O::ModeFrequencyEvaluation,
        O::ModeShapeEvaluation,
        O::DampingRateCalculation,
        O::ModeContributionCalculation,
        O::ModeSum,
        O::ReceivedSeriesSamplingAccumulation,
        O::TranscendentalEvaluationFem,
        O::ModalRhsEvaluation,
        O::InitialStateNumericalConstruction,
        O::OdeIntegration,
        O::EnergyObservation,
        O::DddRegressionEstimation,
        O::SeaRegressionEstimation,
        O::HeldOutPredictionComparison,
    ];

    let product = product();
    let allocation = product.allocation();
    assert_eq!(allocation.exact_inputs(), EXACT_INPUTS);
    assert_eq!(allocation.exact_decisions(), EXACT_DECISIONS);
    assert_eq!(allocation.numerical_operations(), NUMERICAL_OPERATIONS);
    assert!(allocation
        .exact_inputs()
        .windows(2)
        .all(|pair| pair[0] != pair[1]));
    assert!(allocation
        .exact_decisions()
        .windows(2)
        .all(|pair| pair[0] != pair[1]));
    assert!(allocation
        .numerical_operations()
        .windows(2)
        .all(|pair| pair[0] != pair[1]));
}
