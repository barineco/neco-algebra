use neco_algnum::{AlgnumError, RealAlgebraic, RepresentationResource};
use neco_bigint::{BigInt, BigUint, RawRational, ReducedRational};
use neco_expr::{
    AbsoluteBits, Assignments, AtomId, AtomStore, ConsumerId, EvalError, ExactLayer, ExactValue,
    ExprGraph, ExprId, ExprNode, PrecisionRequirements, Resolver,
};
use neco_formsum::{DimensionResource, FormSum, FormSumErrorKind};
use neco_monomial::{Monomial, RawMonomial, RawPower};

fn rational(numerator: i32, denominator: u32) -> ReducedRational {
    RawRational::new(
        BigInt::try_from(numerator).unwrap(),
        BigUint::try_from(denominator).unwrap(),
    )
    .reduce()
    .unwrap()
    .into_reduced()
}

fn layer_values() -> [ExactValue; 3] {
    let form = FormSum::one().unwrap();
    [
        ExactValue::Monomial(Monomial::one()),
        ExactValue::FormSum(form.try_clone().unwrap()),
        ExactValue::Algebraic(RealAlgebraic::from_form_sum(&form).unwrap()),
    ]
}

fn monomial(sign: bool, powers: &[(u32, i32)]) -> Monomial {
    let powers = powers
        .iter()
        .map(|&(base, power)| {
            RawPower::new(
                BigUint::try_from(base).unwrap(),
                RawRational::new(BigInt::try_from(power).unwrap(), BigUint::one().unwrap()),
            )
        })
        .collect();
    if sign {
        RawMonomial::positive(powers)
    } else {
        RawMonomial::negative(powers)
    }
    .normalize()
    .unwrap()
}

fn at_layer(value: Monomial, layer: ExactLayer) -> ExactValue {
    match layer {
        ExactLayer::Monomial => ExactValue::Monomial(value),
        ExactLayer::FormSum => ExactValue::FormSum(FormSum::from_monomial(&value).unwrap()),
        ExactLayer::Algebraic => {
            let form = FormSum::from_monomial(&value).unwrap();
            ExactValue::Algebraic(RealAlgebraic::from_form_sum(&form).unwrap())
        }
    }
}

fn integer_at_layer(value: u32, layer: ExactLayer) -> ExactValue {
    at_layer(monomial(true, &[(value, 1)]), layer)
}

fn fixture() -> (ExprGraph, AtomStore, [ExprId; 3]) {
    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    let mut ids = [ExprId::new(0); 3];
    for (index, value) in layer_values().into_iter().enumerate() {
        let raw = u32::try_from(index).unwrap();
        atoms.insert(AtomId::new(raw), value).unwrap();
        ids[index] = graph.push(ExprNode::Atom(AtomId::new(raw))).unwrap();
    }
    (graph, atoms, ids)
}

fn evaluate_layers(
    graph: &ExprGraph,
    atoms: &AtomStore,
    expressions: &[ExprId],
) -> Vec<ExactLayer> {
    let mut requirements = PrecisionRequirements::new();
    let mut assignments = Assignments::new();
    for (index, expression) in expressions.iter().copied().enumerate() {
        let consumer = ConsumerId::new(u32::try_from(index).unwrap());
        requirements.insert(consumer, AbsoluteBits::new(0)).unwrap();
        assignments.insert(consumer, expression).unwrap();
    }
    let (evaluation, _, resolved) = Resolver::new()
        .resolve_all(graph, atoms, &requirements, &assignments)
        .unwrap();
    for index in 0..expressions.len() {
        assert!(resolved
            .get(ConsumerId::new(u32::try_from(index).unwrap()))
            .unwrap()
            .is_ok());
    }
    expressions
        .iter()
        .map(|id| evaluation.get(*id).unwrap().as_ref().unwrap().layer())
        .collect()
}

#[test]
fn graph_evaluation_routes_every_operation_to_the_exact_normal_value() {
    let layers = [
        ExactLayer::Monomial,
        ExactLayer::FormSum,
        ExactLayer::Algebraic,
    ];
    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    let mut twos = [ExprId::new(0); 3];
    let mut threes = [ExprId::new(0); 3];
    for (index, layer) in layers.into_iter().enumerate() {
        let two_atom = AtomId::new(u32::try_from(index * 2).unwrap());
        let three_atom = AtomId::new(u32::try_from(index * 2 + 1).unwrap());
        atoms.insert(two_atom, integer_at_layer(2, layer)).unwrap();
        atoms
            .insert(three_atom, integer_at_layer(3, layer))
            .unwrap();
        twos[index] = graph.push(ExprNode::Atom(two_atom)).unwrap();
        threes[index] = graph.push(ExprNode::Atom(three_atom)).unwrap();
    }

    let mut cases = Vec::new();
    for (left_index, left_layer) in layers.into_iter().enumerate() {
        for (right_index, right_layer) in layers.into_iter().enumerate() {
            let add_sub =
                if left_layer == ExactLayer::Algebraic || right_layer == ExactLayer::Algebraic {
                    ExactLayer::Algebraic
                } else {
                    ExactLayer::FormSum
                };
            let mul_div = if left_layer == ExactLayer::Algebraic
                || right_layer == ExactLayer::Algebraic
            {
                ExactLayer::Algebraic
            } else if left_layer == ExactLayer::Monomial && right_layer == ExactLayer::Monomial {
                ExactLayer::Monomial
            } else {
                ExactLayer::FormSum
            };
            for (node, expected) in [
                (
                    ExprNode::Add(twos[left_index], threes[right_index]),
                    at_layer(monomial(true, &[(5, 1)]), add_sub),
                ),
                (
                    ExprNode::Sub(twos[left_index], threes[right_index]),
                    at_layer(monomial(false, &[]), add_sub),
                ),
                (
                    ExprNode::Mul(twos[left_index], threes[right_index]),
                    at_layer(monomial(true, &[(2, 1), (3, 1)]), mul_div),
                ),
                (
                    ExprNode::Div(twos[left_index], threes[right_index]),
                    at_layer(monomial(true, &[(2, 1), (3, -1)]), mul_div),
                ),
            ] {
                cases.push((graph.push(node).unwrap(), expected));
            }
        }
    }
    let integer = rational(2, 1);
    let rational_power = rational(1, 2);
    for (index, layer) in layers.into_iter().enumerate() {
        cases.push((
            graph.push(ExprNode::Neg(twos[index])).unwrap(),
            at_layer(monomial(false, &[(2, 1)]), layer),
        ));
        cases.push((
            graph
                .push(ExprNode::Pow {
                    base: twos[index],
                    exponent: integer.try_clone().unwrap(),
                })
                .unwrap(),
            integer_at_layer(4, layer),
        ));
        let result_layer = if layer == ExactLayer::Monomial {
            ExactLayer::Monomial
        } else {
            ExactLayer::Algebraic
        };
        cases.push((
            graph
                .push(ExprNode::Pow {
                    base: twos[index],
                    exponent: rational_power.try_clone().unwrap(),
                })
                .unwrap(),
            at_layer(
                monomial(true, &[(2, 1)]).pow(&rational_power).unwrap(),
                result_layer,
            ),
        ));
    }

    let mut requirements = PrecisionRequirements::new();
    let mut assignments = Assignments::new();
    for (index, (expression, _)) in cases.iter().enumerate() {
        let consumer = ConsumerId::new(u32::try_from(index).unwrap());
        requirements.insert(consumer, AbsoluteBits::new(0)).unwrap();
        assignments.insert(consumer, *expression).unwrap();
    }
    let (evaluation, _, resolved) = Resolver::new()
        .resolve_all(&graph, &atoms, &requirements, &assignments)
        .unwrap();
    assert_eq!(resolved.len(), cases.len());
    for (expression, expected) in &cases {
        assert_eq!(
            evaluation.get(*expression).unwrap().as_ref().unwrap(),
            expected
        );
    }
}

#[test]
fn negation_and_lower_layer_operations_use_the_declared_result_layers() {
    let (mut graph, atoms, ids) = fixture();
    let mut negations = Vec::new();
    for id in ids.into_iter().take(2) {
        negations.push(graph.push(ExprNode::Neg(id)).unwrap());
    }
    assert_eq!(
        evaluate_layers(&graph, &atoms, &negations),
        vec![ExactLayer::Monomial, ExactLayer::FormSum]
    );

    let representatives = [
        graph.push(ExprNode::Add(ids[0], ids[0])).unwrap(),
        graph.push(ExprNode::Sub(ids[1], ids[0])).unwrap(),
        graph.push(ExprNode::Mul(ids[0], ids[0])).unwrap(),
        graph.push(ExprNode::Div(ids[1], ids[0])).unwrap(),
    ];
    assert_eq!(
        evaluate_layers(&graph, &atoms, &representatives),
        vec![
            ExactLayer::FormSum,
            ExactLayer::FormSum,
            ExactLayer::Monomial,
            ExactLayer::FormSum,
        ]
    );
}

#[test]
fn integer_and_true_rational_powers_choose_the_specified_layers() {
    let (mut graph, atoms, ids) = fixture();
    let integer = ids.map(|base| {
        graph
            .push(ExprNode::Pow {
                base,
                exponent: rational(2, 1),
            })
            .unwrap()
    });
    assert_eq!(
        evaluate_layers(&graph, &atoms, &integer),
        vec![
            ExactLayer::Monomial,
            ExactLayer::FormSum,
            ExactLayer::Algebraic
        ]
    );

    let root = ids.map(|base| {
        graph
            .push(ExprNode::Pow {
                base,
                exponent: rational(1, 2),
            })
            .unwrap()
    });
    assert_eq!(
        evaluate_layers(&graph, &atoms, &root),
        vec![
            ExactLayer::Monomial,
            ExactLayer::Algebraic,
            ExactLayer::Algebraic
        ]
    );
}

#[test]
fn structural_power_and_division_failures_have_exact_variants() {
    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    atoms
        .insert(AtomId::new(0), ExactValue::Monomial(Monomial::zero()))
        .unwrap();
    let zero = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let nodes = [
        graph.push(ExprNode::Div(zero, zero)).unwrap(),
        graph
            .push(ExprNode::Pow {
                base: zero,
                exponent: rational(0, 1),
            })
            .unwrap(),
        graph
            .push(ExprNode::Pow {
                base: zero,
                exponent: rational(-1, 1),
            })
            .unwrap(),
    ];
    let mut requirements = PrecisionRequirements::new();
    let mut assignments = Assignments::new();
    for (index, node) in nodes.into_iter().enumerate() {
        let consumer = ConsumerId::new(u32::try_from(index).unwrap());
        requirements.insert(consumer, AbsoluteBits::new(0)).unwrap();
        assignments.insert(consumer, node).unwrap();
    }
    let (evaluation, _, resolved) = Resolver::new()
        .resolve_all(&graph, &atoms, &requirements, &assignments)
        .unwrap();
    assert_eq!(
        evaluation.get(nodes[0]),
        Some(&Err(EvalError::DivisionByZero))
    );
    assert_eq!(
        evaluation.get(nodes[1]),
        Some(&Err(EvalError::UndefinedZeroPower))
    );
    assert_eq!(
        evaluation.get(nodes[2]),
        Some(&Err(EvalError::ZeroToNegativePower))
    );
    assert_eq!(
        resolved.get(ConsumerId::new(0)),
        Some(&Err(neco_expr::ResolveError::Evaluation(
            EvalError::DivisionByZero
        )))
    );
}

#[test]
fn negative_even_root_and_lower_representation_failures_keep_payloads() {
    let maximum_denominator = BigUint::try_from(usize::MAX).unwrap();
    let required_denominator = maximum_denominator.add(&BigUint::one().unwrap()).unwrap();
    let huge_monomial = RawMonomial::positive(vec![RawPower::new(
        BigUint::try_from(2_u8).unwrap(),
        RawRational::new(
            BigInt::one().unwrap(),
            required_denominator.try_clone().unwrap(),
        ),
    )])
    .normalize()
    .unwrap();
    let huge_form = FormSum::from_monomial(&huge_monomial).unwrap();
    let ordinary_monomial = RawMonomial::positive(vec![RawPower::new(
        BigUint::try_from(3_u8).unwrap(),
        RawRational::new(BigInt::one().unwrap(), BigUint::try_from(2_u8).unwrap()),
    )])
    .normalize()
    .unwrap();
    let ordinary_form = FormSum::from_monomial(&ordinary_monomial)
        .unwrap()
        .add(&FormSum::one().unwrap())
        .unwrap();
    let algebraic_form = FormSum::one().unwrap();
    let algebraic = RealAlgebraic::from_form_sum(&algebraic_form).unwrap();
    let root_required = BigUint::try_from(u32::MAX)
        .unwrap()
        .add(&BigUint::one().unwrap())
        .unwrap();
    let root_maximum = BigUint::try_from(u32::MAX).unwrap();
    let root_exponent =
        RawRational::new(BigInt::one().unwrap(), root_required.try_clone().unwrap())
            .reduce()
            .unwrap()
            .into_reduced();

    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    atoms
        .insert(
            AtomId::new(0),
            ExactValue::Monomial(RawMonomial::negative(Vec::new()).normalize().unwrap()),
        )
        .unwrap();
    atoms
        .insert(AtomId::new(1), ExactValue::FormSum(huge_form))
        .unwrap();
    atoms
        .insert(AtomId::new(2), ExactValue::FormSum(ordinary_form))
        .unwrap();
    atoms
        .insert(AtomId::new(3), ExactValue::Algebraic(algebraic))
        .unwrap();
    let negative = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let huge = graph.push(ExprNode::Atom(AtomId::new(1))).unwrap();
    let ordinary = graph.push(ExprNode::Atom(AtomId::new(2))).unwrap();
    let algebraic_id = graph.push(ExprNode::Atom(AtomId::new(3))).unwrap();
    let failures = [
        graph
            .push(ExprNode::Pow {
                base: negative,
                exponent: rational(1, 2),
            })
            .unwrap(),
        graph.push(ExprNode::Div(ordinary, huge)).unwrap(),
        graph
            .push(ExprNode::Pow {
                base: algebraic_id,
                exponent: root_exponent,
            })
            .unwrap(),
    ];
    let mut requirements = PrecisionRequirements::new();
    let mut assignments = Assignments::new();
    for (index, expression) in failures.into_iter().enumerate() {
        let consumer = ConsumerId::new(u32::try_from(index).unwrap());
        requirements.insert(consumer, AbsoluteBits::new(0)).unwrap();
        assignments.insert(consumer, expression).unwrap();
    }
    let (evaluation, _, _) = Resolver::new()
        .resolve_all(&graph, &atoms, &requirements, &assignments)
        .unwrap();
    assert_eq!(
        evaluation.get(failures[0]),
        Some(&Err(EvalError::EvenRootOfNegative))
    );
    assert_eq!(
        evaluation.get(failures[1]),
        Some(&Err(EvalError::FormSum(
            FormSumErrorKind::DimensionOverflow {
                resource: DimensionResource::Denominator,
                required: required_denominator,
                maximum: maximum_denominator,
            }
        )))
    );
    assert_eq!(
        evaluation.get(failures[2]),
        Some(&Err(EvalError::Algnum(AlgnumError::RepresentationLimit {
            resource: RepresentationResource::RootDegree,
            required: root_required,
            maximum: root_maximum,
        })))
    );
}
