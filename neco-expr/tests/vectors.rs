use neco_algnum::RealAlgebraic;
use neco_bigint::{BigInt, BigUint, Dyadic, RawRational, ReducedRational, Sign};
use neco_expr::{
    AbsoluteBits, Assignments, AtomId, AtomStore, ConsumerId, ExactLayer, ExactValue, ExprGraph,
    ExprId, ExprNode, PrecisionRequirements, ResolveError, Resolver,
};
use neco_formsum::{RawFormSum, RawTerm};
use neco_monomial::{Monomial, RawMonomial, RawPower};

fn rational_form(numerator: BigInt, denominator: BigUint) -> neco_formsum::FormSum {
    RawFormSum::new(vec![RawTerm::new(
        RawRational::new(numerator, denominator),
        RawMonomial::positive(Vec::new()),
    )])
    .normalize()
    .unwrap()
}

fn dyadic_form(sign: Sign, magnitude: u64, exponent: usize) -> neco_formsum::FormSum {
    rational_form(
        BigInt::from_sign_magnitude(sign, BigUint::try_from(magnitude).unwrap()),
        BigUint::one().unwrap().shl_bits(exponent).unwrap(),
    )
}

fn exponent(numerator: i32, denominator: u32) -> ReducedRational {
    RawRational::new(
        BigInt::try_from(numerator).unwrap(),
        BigUint::try_from(denominator).unwrap(),
    )
    .reduce()
    .unwrap()
    .into_reduced()
}

fn sqrt_two_form() -> neco_formsum::FormSum {
    let monomial = RawMonomial::positive(vec![RawPower::new(
        BigUint::try_from(2_u8).unwrap(),
        RawRational::new(BigInt::one().unwrap(), BigUint::try_from(2_u8).unwrap()),
    )])
    .normalize()
    .unwrap();
    neco_formsum::FormSum::from_monomial(&monomial).unwrap()
}

fn form_from_dyadic(value: &Dyadic) -> neco_formsum::FormSum {
    let denominator = BigUint::one()
        .unwrap()
        .shl_bits(value.exponent() as usize)
        .unwrap();
    rational_form(value.integer().try_clone().unwrap(), denominator)
}

fn abs_dyadic(value: Dyadic) -> Dyadic {
    Dyadic::new(
        BigInt::from_sign_magnitude(Sign::Positive, value.integer().abs().unwrap()),
        value.exponent(),
    )
}

fn evaluated_layer(graph: &ExprGraph, atoms: &AtomStore, expression: ExprId) -> ExactLayer {
    let mut requirements = PrecisionRequirements::new();
    requirements
        .insert(ConsumerId::new(0), AbsoluteBits::new(0))
        .unwrap();
    let mut assignments = Assignments::new();
    assignments.insert(ConsumerId::new(0), expression).unwrap();
    let (evaluation, _, resolved) = Resolver::new()
        .resolve_all(graph, atoms, &requirements, &assignments)
        .unwrap();
    assert!(resolved.get(ConsumerId::new(0)).unwrap().is_ok());
    evaluation
        .get(expression)
        .unwrap()
        .as_ref()
        .unwrap()
        .layer()
}

#[test]
fn v1_lower_layer_addition_and_integer_power_return_form_sums() {
    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    atoms
        .insert(AtomId::new(0), ExactValue::Monomial(Monomial::one()))
        .unwrap();
    atoms
        .insert(
            AtomId::new(1),
            ExactValue::FormSum(dyadic_form(Sign::Positive, 2, 0)),
        )
        .unwrap();
    let monomial = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let form = graph.push(ExprNode::Atom(AtomId::new(1))).unwrap();
    let addition = graph.push(ExprNode::Add(monomial, monomial)).unwrap();
    let square = graph
        .push(ExprNode::Pow {
            base: form,
            exponent: exponent(2, 1),
        })
        .unwrap();
    assert_eq!(
        evaluated_layer(&graph, &atoms, addition),
        ExactLayer::FormSum
    );
    assert_eq!(evaluated_layer(&graph, &atoms, square), ExactLayer::FormSum);
}

#[test]
fn v2_true_form_sum_root_and_algebraic_product_return_algebraic_values() {
    let form = dyadic_form(Sign::Positive, 4, 0);
    let algebraic = RealAlgebraic::from_form_sum(&form).unwrap();
    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    atoms
        .insert(AtomId::new(0), ExactValue::FormSum(form))
        .unwrap();
    atoms
        .insert(AtomId::new(1), ExactValue::Algebraic(algebraic))
        .unwrap();
    atoms
        .insert(AtomId::new(2), ExactValue::Monomial(Monomial::one()))
        .unwrap();
    let form_id = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let algebraic_id = graph.push(ExprNode::Atom(AtomId::new(1))).unwrap();
    let monomial_id = graph.push(ExprNode::Atom(AtomId::new(2))).unwrap();
    let root = graph
        .push(ExprNode::Pow {
            base: form_id,
            exponent: exponent(1, 2),
        })
        .unwrap();
    let product = graph
        .push(ExprNode::Mul(algebraic_id, monomial_id))
        .unwrap();
    assert_eq!(evaluated_layer(&graph, &atoms, root), ExactLayer::Algebraic);
    assert_eq!(
        evaluated_layer(&graph, &atoms, product),
        ExactLayer::Algebraic
    );
}

fn resolve_atoms(values: Vec<ExactValue>, bits: u32) -> Vec<Result<u64, ResolveError>> {
    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    let mut requirements = PrecisionRequirements::new();
    let mut assignments = Assignments::new();
    for (index, value) in values.into_iter().enumerate() {
        let id = u32::try_from(index).unwrap();
        atoms.insert(AtomId::new(id), value).unwrap();
        let expr = graph.push(ExprNode::Atom(AtomId::new(id))).unwrap();
        requirements
            .insert(ConsumerId::new(id), AbsoluteBits::new(bits))
            .unwrap();
        assignments.insert(ConsumerId::new(id), expr).unwrap();
    }
    let (_, _, resolved) = Resolver::new()
        .resolve_all(&graph, &atoms, &requirements, &assignments)
        .unwrap();
    (0..resolved.len())
        .map(|index| {
            resolved
                .get(ConsumerId::new(u32::try_from(index).unwrap()))
                .unwrap()
                .as_ref()
                .map(|value| value.value().to_bits())
                .map_err(|error| match error {
                    ResolveError::FloatOutOfRange { consumer, expr } => {
                        ResolveError::FloatOutOfRange {
                            consumer: *consumer,
                            expr: *expr,
                        }
                    }
                    _ => panic!("unexpected non-copy error: {error:?}"),
                })
        })
        .collect()
}

#[test]
fn rounds_midpoints_subnormals_and_negative_zero_to_even() {
    let values = vec![
        ExactValue::FormSum(dyadic_form(Sign::Positive, (1_u64 << 54) + 1, 54)),
        ExactValue::FormSum(dyadic_form(Sign::Positive, (1_u64 << 53) + 1, 53)),
        ExactValue::FormSum(dyadic_form(Sign::Positive, (1_u64 << 54) + 3, 54)),
        ExactValue::FormSum(dyadic_form(Sign::Positive, 1, 1075)),
        ExactValue::FormSum(dyadic_form(Sign::Negative, 1, 1075)),
        ExactValue::FormSum(dyadic_form(Sign::Positive, 3, 1075)),
        ExactValue::FormSum(dyadic_form(Sign::Positive, 1, 1074)),
    ];
    assert_eq!(
        resolve_atoms(values, 12),
        vec![
            Ok(1.0_f64.to_bits()),
            Ok(1.0_f64.to_bits()),
            Ok(1.0_f64.to_bits() + 1),
            Ok(0),
            Ok(0),
            Ok(2),
            Ok(1),
        ]
    );
}

#[test]
fn finite_endpoint_is_accepted_and_larger_value_is_local_error() {
    let maximum_magnitude = BigUint::try_from((1_u64 << 53) - 1)
        .unwrap()
        .shl_bits(971)
        .unwrap();
    let maximum = rational_form(
        BigInt::from_sign_magnitude(Sign::Positive, maximum_magnitude.try_clone().unwrap()),
        BigUint::one().unwrap(),
    );
    let above = rational_form(
        BigInt::from_sign_magnitude(
            Sign::Positive,
            maximum_magnitude.add(&BigUint::one().unwrap()).unwrap(),
        ),
        BigUint::one().unwrap(),
    );
    assert_eq!(
        resolve_atoms(
            vec![ExactValue::FormSum(maximum), ExactValue::FormSum(above)],
            0,
        ),
        vec![
            Ok(f64::MAX.to_bits()),
            Err(ResolveError::FloatOutOfRange {
                consumer: ConsumerId::new(1),
                expr: ExprId::new(1),
            }),
        ]
    );
}

#[test]
fn requirements_define_domain_and_consumer_failures_are_isolated() {
    let mut graph = ExprGraph::new();
    let known = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let unknown_atom = graph.push(ExprNode::Atom(AtomId::new(8))).unwrap();
    let mut atoms = AtomStore::new();
    atoms
        .insert(
            AtomId::new(0),
            ExactValue::FormSum(dyadic_form(Sign::Positive, 1, 0)),
        )
        .unwrap();
    let mut requirements = PrecisionRequirements::new();
    let mut assignments = Assignments::new();
    for id in 0..4 {
        requirements
            .insert(ConsumerId::new(id), AbsoluteBits::new(20))
            .unwrap();
    }
    assignments
        .insert(ConsumerId::new(1), ExprId::new(90))
        .unwrap();
    assignments
        .insert(ConsumerId::new(2), unknown_atom)
        .unwrap();
    assignments.insert(ConsumerId::new(3), known).unwrap();
    assignments.insert(ConsumerId::new(99), known).unwrap();

    let (evaluation, isolation, resolved) = Resolver::new()
        .resolve_all(&graph, &atoms, &requirements, &assignments)
        .unwrap();
    assert_eq!(resolved.len(), 4);
    assert_eq!(isolation.len(), 0);
    assert_eq!(
        resolved.get(ConsumerId::new(0)),
        Some(&Err(ResolveError::MissingAssignment {
            consumer: ConsumerId::new(0)
        }))
    );
    assert_eq!(
        resolved.get(ConsumerId::new(1)),
        Some(&Err(ResolveError::UnknownExprId {
            consumer: ConsumerId::new(1),
            expr: ExprId::new(90),
        }))
    );
    assert_eq!(
        resolved.get(ConsumerId::new(2)),
        Some(&Err(ResolveError::UnknownAtomId {
            expr: unknown_atom,
            atom: AtomId::new(8),
        }))
    );
    assert_eq!(
        resolved
            .get(ConsumerId::new(3))
            .unwrap()
            .as_ref()
            .unwrap()
            .value(),
        1.0
    );
    assert_eq!(evaluation.len(), 1);
}

#[test]
fn algebraic_isolation_is_shared_only_for_equal_expression_and_precision() {
    let form = dyadic_form(Sign::Positive, 1, 1);
    let algebraic = RealAlgebraic::from_form_sum(&form).unwrap();
    let mut graph = ExprGraph::new();
    let expr = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let mut atoms = AtomStore::new();
    atoms
        .insert(AtomId::new(0), ExactValue::Algebraic(algebraic))
        .unwrap();
    let mut requirements = PrecisionRequirements::new();
    requirements
        .insert(ConsumerId::new(0), AbsoluteBits::new(20))
        .unwrap();
    requirements
        .insert(ConsumerId::new(1), AbsoluteBits::new(20))
        .unwrap();
    requirements
        .insert(ConsumerId::new(2), AbsoluteBits::new(40))
        .unwrap();
    let mut assignments = Assignments::new();
    for id in 0..3 {
        assignments.insert(ConsumerId::new(id), expr).unwrap();
    }

    let (_, isolation, resolved) = Resolver::new()
        .resolve_all(&graph, &atoms, &requirements, &assignments)
        .unwrap();
    assert_eq!(resolved.len(), 3);
    assert_eq!(isolation.len(), 2);
    assert!(isolation.get(expr, AbsoluteBits::new(20)).is_some());
    assert!(isolation.get(expr, AbsoluteBits::new(40)).is_some());
    let target = Dyadic::new(BigInt::one().unwrap(), 20);
    assert!(
        resolved
            .get(ConsumerId::new(0))
            .unwrap()
            .as_ref()
            .unwrap()
            .enclosure()
            .width()
            .unwrap()
            <= target
    );
}

#[test]
fn v8_changing_only_the_high_precision_requirement_preserves_the_other_result() {
    let mut graph = ExprGraph::new();
    let expr = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let mut atoms = AtomStore::new();
    atoms
        .insert(
            AtomId::new(0),
            ExactValue::Algebraic(RealAlgebraic::from_form_sum(&sqrt_two_form()).unwrap()),
        )
        .unwrap();
    let mut requirements = PrecisionRequirements::new();
    requirements
        .insert(ConsumerId::new(20), AbsoluteBits::new(20))
        .unwrap();
    requirements
        .insert(ConsumerId::new(80), AbsoluteBits::new(80))
        .unwrap();
    let mut assignments = Assignments::new();
    assignments.insert(ConsumerId::new(20), expr).unwrap();
    assignments.insert(ConsumerId::new(80), expr).unwrap();

    let graph_before = graph.try_clone().unwrap();
    let atoms_before = atoms.try_clone().unwrap();
    let assignments_before = assignments.try_clone().unwrap();
    let (evaluation_before, isolation_before, before) = Resolver::new()
        .resolve_all(&graph, &atoms, &requirements, &assignments)
        .unwrap();
    requirements
        .set(ConsumerId::new(80), AbsoluteBits::new(96))
        .unwrap();
    let (evaluation_after, isolation_after, after) = Resolver::new()
        .resolve_all(&graph, &atoms, &requirements, &assignments)
        .unwrap();
    assert_eq!(graph, graph_before);
    assert_eq!(atoms, atoms_before);
    assert_eq!(assignments, assignments_before);
    assert_eq!(evaluation_before, evaluation_after);
    assert_ne!(isolation_before, isolation_after);
    assert_eq!(isolation_before.len(), 2);
    assert_eq!(isolation_after.len(), 2);
    assert_eq!(
        isolation_before.get(expr, AbsoluteBits::new(20)),
        isolation_after.get(expr, AbsoluteBits::new(20))
    );
    assert!(isolation_before.get(expr, AbsoluteBits::new(80)).is_some());
    assert!(isolation_before.get(expr, AbsoluteBits::new(96)).is_none());
    assert!(isolation_after.get(expr, AbsoluteBits::new(80)).is_none());
    assert!(isolation_after.get(expr, AbsoluteBits::new(96)).is_some());
    assert_eq!(
        isolation_before
            .get(expr, AbsoluteBits::new(80))
            .unwrap()
            .enclosure(),
        before
            .get(ConsumerId::new(80))
            .unwrap()
            .as_ref()
            .unwrap()
            .enclosure()
    );
    assert_eq!(
        isolation_after
            .get(expr, AbsoluteBits::new(96))
            .unwrap()
            .enclosure(),
        after
            .get(ConsumerId::new(80))
            .unwrap()
            .as_ref()
            .unwrap()
            .enclosure()
    );
    assert_eq!(
        before.get(ConsumerId::new(20)),
        after.get(ConsumerId::new(20))
    );
    assert_ne!(
        before
            .get(ConsumerId::new(80))
            .unwrap()
            .as_ref()
            .unwrap()
            .enclosure(),
        after
            .get(ConsumerId::new(80))
            .unwrap()
            .as_ref()
            .unwrap()
            .enclosure()
    );
}

#[test]
fn successful_certificates_recompute_containment_width_and_both_endpoint_error() {
    let floats = [
        -f64::MAX,
        -1.0,
        -f64::from_bits(1),
        0.0,
        f64::from_bits(1),
        1.0,
        f64::MAX,
    ];
    let exact = floats
        .into_iter()
        .map(|value| Dyadic::from_f64_exact(value).unwrap())
        .collect::<Vec<_>>();
    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    let mut requirements = PrecisionRequirements::new();
    let mut assignments = Assignments::new();
    for (index, value) in exact.iter().enumerate() {
        let raw = u32::try_from(index).unwrap();
        atoms
            .insert(
                AtomId::new(raw),
                ExactValue::FormSum(form_from_dyadic(value)),
            )
            .unwrap();
        let expression = graph.push(ExprNode::Atom(AtomId::new(raw))).unwrap();
        requirements
            .insert(ConsumerId::new(raw), AbsoluteBits::new(20))
            .unwrap();
        assignments
            .insert(ConsumerId::new(raw), expression)
            .unwrap();
    }
    let (_, _, resolved) = Resolver::new()
        .resolve_all(&graph, &atoms, &requirements, &assignments)
        .unwrap();
    let maximum_width = Dyadic::new(BigInt::one().unwrap(), 20);
    for (index, target) in exact.iter().enumerate() {
        let certificate = resolved
            .get(ConsumerId::new(u32::try_from(index).unwrap()))
            .unwrap()
            .as_ref()
            .unwrap();
        assert!(certificate.enclosure().lower() <= target);
        assert!(target <= certificate.enclosure().upper());
        let width = certificate.enclosure().width().unwrap();
        assert_ne!(width.cmp(&maximum_width), core::cmp::Ordering::Greater);
        let selected = Dyadic::from_f64_exact(certificate.value()).unwrap();
        let lower = abs_dyadic(selected.sub(certificate.enclosure().lower()).unwrap());
        let upper = abs_dyadic(certificate.enclosure().upper().sub(&selected).unwrap());
        let recomputed = if lower >= upper { lower } else { upper };
        assert_eq!(certificate.absolute_error(), &recomputed);
    }
}

#[test]
fn non_dyadic_certificates_observe_precision_and_both_asymmetric_endpoint_distances() {
    let target = sqrt_two_form();
    let mut graph = ExprGraph::new();
    let positive = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let negative = graph.push(ExprNode::Neg(positive)).unwrap();
    let mut atoms = AtomStore::new();
    atoms
        .insert(
            AtomId::new(0),
            ExactValue::FormSum(target.try_clone().unwrap()),
        )
        .unwrap();
    let mut requirements = PrecisionRequirements::new();
    let mut assignments = Assignments::new();
    for (consumer, expression) in [
        (ConsumerId::new(0), positive),
        (ConsumerId::new(1), negative),
    ] {
        requirements
            .insert(consumer, AbsoluteBits::new(20))
            .unwrap();
        assignments.insert(consumer, expression).unwrap();
    }
    let (_, _, resolved) = Resolver::new()
        .resolve_all(&graph, &atoms, &requirements, &assignments)
        .unwrap();
    let maximum_width = Dyadic::new(BigInt::one().unwrap(), 20);
    let mut lower_is_greater = false;
    let mut upper_is_greater = false;
    for consumer in [ConsumerId::new(0), ConsumerId::new(1)] {
        let certificate = resolved.get(consumer).unwrap().as_ref().unwrap();
        let width = certificate.enclosure().width().unwrap();
        assert_ne!(width.cmp(&maximum_width), core::cmp::Ordering::Greater);
        assert!(!width.integer().is_zero());
        let selected = Dyadic::from_f64_exact(certificate.value()).unwrap();
        let lower = abs_dyadic(selected.sub(certificate.enclosure().lower()).unwrap());
        let upper = abs_dyadic(certificate.enclosure().upper().sub(&selected).unwrap());
        lower_is_greater |= lower > upper;
        upper_is_greater |= upper > lower;
        let recomputed = if lower >= upper { lower } else { upper };
        assert_eq!(certificate.absolute_error(), &recomputed);
    }
    assert!(lower_is_greater && upper_is_greater);

    let positive_certificate = resolved.get(ConsumerId::new(0)).unwrap().as_ref().unwrap();
    let lower_form = form_from_dyadic(positive_certificate.enclosure().lower());
    let upper_form = form_from_dyadic(positive_certificate.enclosure().upper());
    assert_ne!(
        target.sub(&lower_form).unwrap().sign().unwrap(),
        Sign::Negative
    );
    assert_ne!(
        upper_form.sub(&target).unwrap().sign().unwrap(),
        Sign::Negative
    );
}
