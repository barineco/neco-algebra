use core::cmp::Ordering;
use neco_algnum::RealAlgebraic;
use neco_bigint::{BigInt, BigUint, Dyadic, RawRational};
use neco_expr::{
    allocate_exact_numeric, assemble_exact_computation_product, decide_exact_properties,
    normalize_exact_expressions, read_exact_numeric_inputs, resolve_certified_f64, AbsoluteBits,
    AtomId, AtomStore, ConsumerId, ExactAllocationInput, ExactComputationProduct,
    ExactDecisionKind, ExactDecisionRequest, ExactDecisionValue, ExactExpressionRequirement,
    ExactOperation, ExactValue, ExprGraph, ExprNode, NecoFailure, NecoImplementationSource,
    NecoObservedCapability, OwnerExpressionSet, OwnerId, ProvenanceMismatchField,
    StorageFailurePayload, StorageResource, UnsupportedFailurePayload,
};
use neco_formsum::FormSum;
use neco_monomial::{Monomial, RawMonomial, RawPower};

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

fn sqrt_two_exact() -> ExactValue {
    let monomial = RawMonomial::positive(vec![RawPower::new(
        BigUint::try_from(2_u8).unwrap(),
        RawRational::new(BigInt::one().unwrap(), BigUint::try_from(2_u8).unwrap()),
    )])
    .normalize()
    .unwrap();
    let form = FormSum::from_monomial(&monomial).unwrap();
    ExactValue::Algebraic(RealAlgebraic::from_form_sum(&form).unwrap())
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

fn input() -> ExactAllocationInput {
    let (first_owner_graph, first_owner_atoms) = domain();
    let first_owner_requirements = (0..5).map(|id| requirement(id, id)).collect();
    let first_owner = OwnerExpressionSet::new(
        OwnerId::new(1),
        first_owner_graph,
        first_owner_atoms,
        first_owner_requirements,
    )
    .unwrap();

    let (second_owner_graph, second_owner_atoms) = domain();
    let second_owner_requirements = (5..12).map(|id| requirement(id, id)).collect();
    let second_owner = OwnerExpressionSet::new(
        OwnerId::new(2),
        second_owner_graph,
        second_owner_atoms,
        second_owner_requirements,
    )
    .unwrap();

    ExactAllocationInput::new(
        vec![first_owner, second_owner],
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

fn irrational_precision_pair() -> ExactComputationProduct {
    let mut graph = ExprGraph::new();
    let expression = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let mut atoms = AtomStore::new();
    atoms.insert(AtomId::new(0), sqrt_two_exact()).unwrap();
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
        AbsoluteBits::new(80),
    )
    .unwrap();
    let first_owner =
        OwnerExpressionSet::new(OwnerId::new(1), graph, atoms, vec![low, high]).unwrap();
    let (second_owner_graph, second_owner_atoms) = domain();
    let second_owner = OwnerExpressionSet::new(
        OwnerId::new(2),
        second_owner_graph,
        second_owner_atoms,
        vec![],
    )
    .unwrap();
    let input = ExactAllocationInput::new(
        vec![first_owner, second_owner],
        NecoObservedCapability::current(&OPERATIONS).unwrap(),
        NecoImplementationSource::current(),
    );
    let allocated = allocate_exact_numeric(read_exact_numeric_inputs(input).unwrap()).unwrap();
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
    for (owner, requirement) in inspection.requirements() {
        let bundle = product
            .certified_descent(*owner, requirement.consumer_id())
            .unwrap();
        assert_eq!(bundle.owner(), *owner);
        assert_eq!(bundle.consumer_id(), requirement.consumer_id());
        assert_eq!(bundle.precision(), requirement.precision());
        assert_eq!(bundle.source(), NecoImplementationSource::current());
        assert_eq!(bundle.values().len(), requirement.expressions().len());
        let maximum_width = Dyadic::new(BigInt::one().unwrap(), requirement.precision().get());
        for value in bundle.values() {
            assert_eq!(value.owner(), *owner);
            assert_eq!(value.consumer_id(), requirement.consumer_id());
            assert_eq!(value.precision(), requirement.precision());
            assert_eq!(value.source(), NecoImplementationSource::current());
            assert!(requirement.expressions().contains(&value.expr()));
            assert!(value.certified().enclosure().width().unwrap() <= maximum_width);
        }
    }
}

#[test]
fn irrational_projection_keeps_rounding_independent_from_enclosure_precision() {
    let exact = sqrt_two_exact();
    let ExactValue::Algebraic(exact) = &exact else {
        unreachable!("sqrt_two_exact returns an algebraic value");
    };
    let product = irrational_precision_pair();
    let low = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(0))
        .unwrap()
        .values()[0]
        .certified();
    let high = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(1))
        .unwrap()
        .values()[0]
        .certified();
    assert_eq!(low.value().to_bits(), high.value().to_bits());
    let low_width = low.enclosure().width().unwrap();
    let high_width = high.enclosure().width().unwrap();
    assert!(high_width < low_width);
    assert!(high_width <= Dyadic::new(BigInt::one().unwrap(), 80));
    for certificate in [low, high] {
        assert_ne!(
            exact
                .compare_dyadic(certificate.enclosure().lower())
                .unwrap(),
            Ordering::Less
        );
        assert_ne!(
            exact
                .compare_dyadic(certificate.enclosure().upper())
                .unwrap(),
            Ordering::Greater
        );
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
        let witness = product
            .decision(OwnerId::new(1), ConsumerId::new(consumer))
            .unwrap();
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
    let (first_owner_graph, first_owner_atoms) = domain();
    let first_owner = OwnerExpressionSet::new(
        OwnerId::new(1),
        first_owner_graph,
        first_owner_atoms,
        vec![requirement(0, 0)],
    )
    .unwrap();
    let (second_owner_graph, second_owner_atoms) = domain();
    let second_owner = OwnerExpressionSet::new(
        OwnerId::new(2),
        second_owner_graph,
        second_owner_atoms,
        vec![],
    )
    .unwrap();
    let input = ExactAllocationInput::new(
        vec![first_owner, second_owner],
        NecoObservedCapability::new("wrong", &OPERATIONS).unwrap(),
        NecoImplementationSource::current(),
    );
    assert!(matches!(
        read_exact_numeric_inputs(input),
        Err(NecoFailure::ProvenanceMismatch {
            location,
            payload,
        }) if location.operation_kind() == neco_expr::FailureOperation::Read
            && payload.field() == ProvenanceMismatchField::Source
    ));
}

#[test]
fn owner_separates_same_expression_id_for_distinct_monomial_atoms() {
    let mut first_owner_graph = ExprGraph::new();
    let expression = first_owner_graph
        .push(ExprNode::Atom(AtomId::new(0)))
        .unwrap();
    let mut first_owner_atoms = AtomStore::new();
    first_owner_atoms
        .insert(AtomId::new(0), ExactValue::Monomial(Monomial::one()))
        .unwrap();
    let mut second_owner_graph = ExprGraph::new();
    assert_eq!(
        second_owner_graph
            .push(ExprNode::Atom(AtomId::new(0)))
            .unwrap(),
        expression
    );
    let mut second_owner_atoms = AtomStore::new();
    second_owner_atoms
        .insert(AtomId::new(0), ExactValue::Monomial(Monomial::zero()))
        .unwrap();
    let first_owner = OwnerExpressionSet::new(
        OwnerId::new(1),
        first_owner_graph,
        first_owner_atoms,
        vec![ExactExpressionRequirement::new(
            ConsumerId::new(0),
            &[expression],
            ExactDecisionRequest::Sign(expression),
            AbsoluteBits::new(30),
        )
        .unwrap()],
    )
    .unwrap();
    let second_owner = OwnerExpressionSet::new(
        OwnerId::new(2),
        second_owner_graph,
        second_owner_atoms,
        vec![ExactExpressionRequirement::new(
            ConsumerId::new(0),
            &[expression],
            ExactDecisionRequest::Zero(expression),
            AbsoluteBits::new(30),
        )
        .unwrap()],
    )
    .unwrap();
    let input = ExactAllocationInput::new(
        vec![first_owner, second_owner],
        NecoObservedCapability::current(&OPERATIONS).unwrap(),
        NecoImplementationSource::current(),
    );
    let product = assemble_exact_computation_product(
        resolve_certified_f64(
            decide_exact_properties(
                normalize_exact_expressions(
                    allocate_exact_numeric(read_exact_numeric_inputs(input).unwrap()).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        product
            .direct_inspection()
            .unwrap()
            .shared_resolution_count(),
        2
    );
    assert!(product
        .allocation()
        .descent_consumers()
        .contains(&(OwnerId::new(1), ConsumerId::new(0))));
    assert!(product
        .allocation()
        .descent_consumers()
        .contains(&(OwnerId::new(2), ConsumerId::new(0))));
    let first_owner_bundle = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(0))
        .unwrap();
    let second_owner_bundle = product
        .certified_descent(OwnerId::new(2), ConsumerId::new(0))
        .unwrap();
    assert_eq!(first_owner_bundle.owner(), OwnerId::new(1));
    assert_eq!(second_owner_bundle.owner(), OwnerId::new(2));
    assert_eq!(first_owner_bundle.consumer_id(), ConsumerId::new(0));
    assert_eq!(second_owner_bundle.consumer_id(), ConsumerId::new(0));
    assert_eq!(first_owner_bundle.values()[0].expr(), expression);
    assert_eq!(second_owner_bundle.values()[0].expr(), expression);
    assert_eq!(first_owner_bundle.values()[0].certified().value(), 1.0);
    assert_eq!(second_owner_bundle.values()[0].certified().value(), 0.0);
    assert!(!product
        .normal_form(OwnerId::new(1), ConsumerId::new(0), expression)
        .unwrap()
        .is_zero());
    assert!(product
        .normal_form(OwnerId::new(2), ConsumerId::new(0), expression)
        .unwrap()
        .is_zero());
    assert_eq!(
        product
            .decision(OwnerId::new(1), ConsumerId::new(0))
            .unwrap()
            .value(),
        ExactDecisionValue::Positive
    );
    assert_eq!(
        product
            .decision(OwnerId::new(2), ConsumerId::new(0))
            .unwrap()
            .value(),
        ExactDecisionValue::Zero
    );
}

#[test]
fn owner_separates_same_expression_id_for_distinct_algebraic_atoms() {
    let mut first_owner_graph = ExprGraph::new();
    let expression = first_owner_graph
        .push(ExprNode::Atom(AtomId::new(0)))
        .unwrap();
    let mut second_owner_graph = ExprGraph::new();
    assert_eq!(
        second_owner_graph
            .push(ExprNode::Atom(AtomId::new(0)))
            .unwrap(),
        expression
    );
    let mut first_owner_atoms = AtomStore::new();
    first_owner_atoms
        .insert(
            AtomId::new(0),
            ExactValue::Algebraic(
                neco_algnum::RealAlgebraic::from_integer(neco_bigint::BigInt::try_from(1).unwrap())
                    .unwrap(),
            ),
        )
        .unwrap();
    let mut second_owner_atoms = AtomStore::new();
    second_owner_atoms
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
    let first_owner = OwnerExpressionSet::new(
        OwnerId::new(1),
        first_owner_graph,
        first_owner_atoms,
        vec![requirement(0)],
    )
    .unwrap();
    let second_owner = OwnerExpressionSet::new(
        OwnerId::new(2),
        second_owner_graph,
        second_owner_atoms,
        vec![requirement(1)],
    )
    .unwrap();
    let input = ExactAllocationInput::new(
        vec![first_owner, second_owner],
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
    let first_owner_certified = &resolved
        .certified_descent(OwnerId::new(1), ConsumerId::new(0))
        .unwrap()
        .values()[0];
    let second_owner_certified = &resolved
        .certified_descent(OwnerId::new(2), ConsumerId::new(1))
        .unwrap()
        .values()[0];
    assert_eq!(first_owner_certified.certified().value(), 1.0);
    assert_eq!(second_owner_certified.certified().value(), 2.0);
    assert_ne!(
        first_owner_certified.certified().enclosure(),
        second_owner_certified.certified().enclosure()
    );
}

#[test]
fn unsupported_required_operation_is_rejected() {
    let (first_owner_graph, first_owner_atoms) = domain();
    let first_owner = OwnerExpressionSet::new(
        OwnerId::new(1),
        first_owner_graph,
        first_owner_atoms,
        vec![requirement(0, 0)],
    )
    .unwrap();
    let (second_owner_graph, second_owner_atoms) = domain();
    let second_owner = OwnerExpressionSet::new(
        OwnerId::new(2),
        second_owner_graph,
        second_owner_atoms,
        vec![],
    )
    .unwrap();
    let input = ExactAllocationInput::new(
        vec![first_owner, second_owner],
        NecoObservedCapability::current(&OPERATIONS[..OPERATIONS.len() - 1]).unwrap(),
        NecoImplementationSource::current(),
    );
    let read = read_exact_numeric_inputs(input).unwrap();
    assert!(matches!(
        allocate_exact_numeric(read),
        Err(NecoFailure::UnsupportedRequiredOperation { payload, .. })
            if matches!(
                *payload,
                UnsupportedFailurePayload::RequiredOperation("resolve-certified-f64")
            )
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
    let first_owner =
        OwnerExpressionSet::new(OwnerId::new(1), graph, atoms, vec![low, high]).unwrap();
    let (second_owner_graph, second_owner_atoms) = domain();
    let second_owner = OwnerExpressionSet::new(
        OwnerId::new(2),
        second_owner_graph,
        second_owner_atoms,
        vec![],
    )
    .unwrap();
    let input = ExactAllocationInput::new(
        vec![first_owner, second_owner],
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
fn invalid_requirement_constructors_reject_empty_missing_operands_and_duplicates() {
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
    let duplicate = ExactExpressionRequirement::new(
        consumer,
        &[zero, zero],
        ExactDecisionRequest::Zero(zero),
        AbsoluteBits::new(20),
    );
    assert!(matches!(
        duplicate,
        Err(NecoFailure::StorageFailure { location, payload })
            if location.operation_kind() == neco_expr::FailureOperation::Allocate
                && location.consumer() == Some(consumer)
                && matches!(
                    *payload,
                    StorageFailurePayload::DuplicateId { resource, id }
                        if resource == StorageResource::ProductEntries && id == zero.get()
                )
    ));
}

fn product_from_owners(owners: Vec<OwnerExpressionSet>) -> ExactComputationProduct {
    let input = ExactAllocationInput::new(
        owners,
        NecoObservedCapability::current(&OPERATIONS).unwrap(),
        NecoImplementationSource::current(),
    );
    let allocated = allocate_exact_numeric(read_exact_numeric_inputs(input).unwrap()).unwrap();
    let normalized = normalize_exact_expressions(allocated).unwrap();
    let decided = decide_exact_properties(normalized).unwrap();
    let resolved = resolve_certified_f64(decided).unwrap();
    assemble_exact_computation_product(resolved).unwrap()
}

fn owner_with_single_atom(
    owner: OwnerId,
    atom: ExactValue,
    consumer: ConsumerId,
    decision: fn(neco_expr::ExprId) -> ExactDecisionRequest,
    precision: u32,
) -> OwnerExpressionSet {
    let mut graph = ExprGraph::new();
    let expression = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let mut atoms = AtomStore::new();
    atoms.insert(AtomId::new(0), atom).unwrap();
    let requirement = ExactExpressionRequirement::new(
        consumer,
        &[expression],
        decision(expression),
        AbsoluteBits::new(precision),
    )
    .unwrap();
    OwnerExpressionSet::new(owner, graph, atoms, vec![requirement]).unwrap()
}

#[test]
fn owner_appearing_twice_is_rejected_with_the_owner_as_duplicate_identifier() {
    let duplicated = OwnerId::new(7);
    let build_input = || {
        let owners = [
            (duplicated, 0),
            (OwnerId::new(8), 1),
            (duplicated, 2),
            (OwnerId::new(9), 3),
        ]
        .into_iter()
        .map(|(owner, consumer)| {
            let (graph, atoms) = domain();
            OwnerExpressionSet::new(owner, graph, atoms, vec![requirement(consumer, consumer)])
                .unwrap()
        })
        .collect();
        ExactAllocationInput::new(
            owners,
            NecoObservedCapability::current(&OPERATIONS).unwrap(),
            NecoImplementationSource::current(),
        )
    };
    let assert_duplicate_owner = |failure: NecoFailure, operation: neco_expr::FailureOperation| {
        assert!(matches!(
            failure,
            NecoFailure::StorageFailure { location, payload }
                if location.operation_kind() == operation
                    && matches!(
                        *payload,
                        StorageFailurePayload::DuplicateId { resource, id }
                            if resource == StorageResource::ProductEntries
                                && id == duplicated.get()
                    )
        ));
    };
    assert_duplicate_owner(
        read_exact_numeric_inputs(build_input()).unwrap_err(),
        neco_expr::FailureOperation::Read,
    );
    assert_duplicate_owner(
        allocate_exact_numeric(build_input()).unwrap_err(),
        neco_expr::FailureOperation::Allocate,
    );
}

#[test]
fn owner_appearing_twice_is_rejected_even_when_both_sets_are_identical() {
    let owner = OwnerId::new(3);
    let identical_set = || {
        let (graph, atoms) = domain();
        OwnerExpressionSet::new(owner, graph, atoms, vec![requirement(0, 0)]).unwrap()
    };
    let input = ExactAllocationInput::new(
        vec![identical_set(), identical_set()],
        NecoObservedCapability::current(&OPERATIONS).unwrap(),
        NecoImplementationSource::current(),
    );
    assert!(matches!(
        read_exact_numeric_inputs(input),
        Err(NecoFailure::StorageFailure { payload, .. })
            if matches!(
                *payload,
                StorageFailurePayload::DuplicateId { id, .. } if id == owner.get()
            )
    ));
}

#[test]
fn shared_consumer_identifier_resolves_each_owner_from_its_own_requirement() {
    let sqrt_two_owner = OwnerId::new(4);
    let zero_owner = OwnerId::new(5);
    let shared_consumer = ConsumerId::new(0);
    let build_owners = || {
        vec![
            owner_with_single_atom(
                sqrt_two_owner,
                sqrt_two_exact(),
                shared_consumer,
                ExactDecisionRequest::Sign,
                20,
            ),
            owner_with_single_atom(
                zero_owner,
                ExactValue::Monomial(Monomial::zero()),
                shared_consumer,
                ExactDecisionRequest::Zero,
                64,
            ),
        ]
    };
    let ascending = product_from_owners(build_owners());
    let descending = product_from_owners(build_owners().into_iter().rev().collect());
    for product in [&ascending, &descending] {
        let sqrt_two_bundle = product
            .certified_descent(sqrt_two_owner, shared_consumer)
            .unwrap();
        let zero_bundle = product
            .certified_descent(zero_owner, shared_consumer)
            .unwrap();
        assert_eq!(sqrt_two_bundle.owner(), sqrt_two_owner);
        assert_eq!(zero_bundle.owner(), zero_owner);
        assert_eq!(sqrt_two_bundle.precision(), AbsoluteBits::new(20));
        assert_eq!(zero_bundle.precision(), AbsoluteBits::new(64));
        assert_eq!(
            sqrt_two_bundle.values()[0].certified().value(),
            core::f64::consts::SQRT_2
        );
        assert_eq!(zero_bundle.values()[0].certified().value(), 0.0);
        let sqrt_two_decision = product.decision(sqrt_two_owner, shared_consumer).unwrap();
        let zero_decision = product.decision(zero_owner, shared_consumer).unwrap();
        assert_eq!(sqrt_two_decision.kind(), ExactDecisionKind::Sign);
        assert_eq!(sqrt_two_decision.value(), ExactDecisionValue::Positive);
        assert_eq!(zero_decision.kind(), ExactDecisionKind::Zero);
        assert_eq!(zero_decision.value(), ExactDecisionValue::Zero);
        assert!(product
            .certified_descent(OwnerId::new(6), shared_consumer)
            .is_none());
        assert_eq!(product.allocation().descent_consumers().len(), 2);
    }
    for owner in [sqrt_two_owner, zero_owner] {
        let ascending_bundle = ascending.certified_descent(owner, shared_consumer).unwrap();
        let descending_bundle = descending
            .certified_descent(owner, shared_consumer)
            .unwrap();
        assert_eq!(
            ascending_bundle.values()[0].certified(),
            descending_bundle.values()[0].certified()
        );
        assert_eq!(
            ascending.decision(owner, shared_consumer).unwrap().value(),
            descending.decision(owner, shared_consumer).unwrap().value()
        );
    }
}

#[test]
fn implementation_source_revision_is_the_neco_expr_package_version() {
    let manifest = include_str!("../Cargo.toml");
    let package_section = manifest
        .split("[package]")
        .nth(1)
        .and_then(|rest| rest.split("\n[").next())
        .unwrap();
    let declared_version = package_section
        .lines()
        .find_map(|line| {
            line.strip_prefix("version = \"")
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .unwrap();
    let source = NecoImplementationSource::current();
    assert_eq!(source.revision_identity(), declared_version);
    let numeric_components: Vec<&str> = source.revision_identity().split('.').collect();
    assert_eq!(numeric_components.len(), 3);
    assert!(numeric_components
        .iter()
        .all(|component| component.parse::<u32>().is_ok()));
    assert_eq!(
        NecoObservedCapability::current(&OPERATIONS)
            .unwrap()
            .source_identity(),
        declared_version
    );
}
