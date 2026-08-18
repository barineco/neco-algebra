use neco_algnum::RealAlgebraic;
use neco_bigint::{BigInt, BigUint, RawRational};
use neco_expr::{
    AbsoluteBits, Assignments, AtomId, AtomStore, ConsumerId, EvaluationCache, ExactValue,
    ExprGraph, ExprId, ExprNode, IsolationCache, PrecisionRequirements, ResolvedValues, Resolver,
};
use neco_formsum::FormSum;
use neco_monomial::{RawMonomial, RawPower};

fn radical_form(base: u32) -> FormSum {
    let monomial = RawMonomial::positive(vec![RawPower::new(
        BigUint::try_from(base).unwrap(),
        RawRational::new(BigInt::one().unwrap(), BigUint::try_from(2_u8).unwrap()),
    )])
    .normalize()
    .unwrap();
    FormSum::from_monomial(&monomial).unwrap()
}

fn resolve(
    graph: &ExprGraph,
    atoms: &AtomStore,
    requirements: &PrecisionRequirements,
    assignments: &Assignments,
) -> (EvaluationCache, IsolationCache, ResolvedValues) {
    Resolver::new()
        .resolve_all(graph, atoms, requirements, assignments)
        .unwrap()
}

fn base_inputs() -> (
    ExprGraph,
    AtomStore,
    PrecisionRequirements,
    Assignments,
    [ExprId; 2],
) {
    let mut graph = ExprGraph::new();
    let positive = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
    let negative = graph.push(ExprNode::Neg(positive)).unwrap();
    let mut atoms = AtomStore::new();
    atoms
        .insert(
            AtomId::new(0),
            ExactValue::Algebraic(RealAlgebraic::from_form_sum(&radical_form(2)).unwrap()),
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
    assignments.insert(ConsumerId::new(20), positive).unwrap();
    assignments.insert(ConsumerId::new(80), positive).unwrap();
    (
        graph,
        atoms,
        requirements,
        assignments,
        [positive, negative],
    )
}

#[test]
fn g_s_d_and_a_updates_are_independent_values() {
    let (graph, atoms, requirements, assignments, expressions) = base_inputs();
    let baseline = resolve(&graph, &atoms, &requirements, &assignments);
    let original_graph = graph.try_clone().unwrap();
    let original_atoms = atoms.try_clone().unwrap();
    let original_requirements = requirements.try_clone().unwrap();
    let original_assignments = assignments.try_clone().unwrap();

    let mut changed_graph = graph.try_clone().unwrap();
    changed_graph.push(ExprNode::Neg(expressions[1])).unwrap();
    assert_eq!(
        resolve(&changed_graph, &atoms, &requirements, &assignments),
        baseline
    );
    assert_ne!(changed_graph, original_graph);
    assert_eq!(atoms, original_atoms);
    assert_eq!(requirements, original_requirements);
    assert_eq!(assignments, original_assignments);

    let mut changed_atoms = atoms.try_clone().unwrap();
    changed_atoms
        .set(
            AtomId::new(0),
            ExactValue::Algebraic(RealAlgebraic::from_form_sum(&radical_form(3)).unwrap()),
        )
        .unwrap();
    let atom_result = resolve(&graph, &changed_atoms, &requirements, &assignments);
    assert_ne!(atom_result.0, baseline.0);
    assert_ne!(atom_result.1, baseline.1);
    assert_ne!(atom_result.2, baseline.2);
    for consumer in [ConsumerId::new(20), ConsumerId::new(80)] {
        assert_ne!(atom_result.2.get(consumer), baseline.2.get(consumer));
    }
    assert_eq!(graph, original_graph);
    assert_ne!(changed_atoms, original_atoms);
    assert_eq!(requirements, original_requirements);
    assert_eq!(assignments, original_assignments);

    let mut changed_requirements = requirements.try_clone().unwrap();
    changed_requirements
        .set(ConsumerId::new(80), AbsoluteBits::new(96))
        .unwrap();
    let precision_result = resolve(&graph, &atoms, &changed_requirements, &assignments);
    assert_eq!(precision_result.0, baseline.0);
    assert_ne!(precision_result.1, baseline.1);
    assert_eq!(
        precision_result
            .1
            .get(expressions[0], AbsoluteBits::new(20)),
        baseline.1.get(expressions[0], AbsoluteBits::new(20))
    );
    assert_eq!(
        precision_result.2.get(ConsumerId::new(20)),
        baseline.2.get(ConsumerId::new(20))
    );
    assert_ne!(
        precision_result.2.get(ConsumerId::new(80)),
        baseline.2.get(ConsumerId::new(80))
    );
    assert_eq!(graph, original_graph);
    assert_eq!(atoms, original_atoms);
    assert_ne!(changed_requirements, original_requirements);
    assert_eq!(assignments, original_assignments);

    let mut changed_assignments = assignments.try_clone().unwrap();
    changed_assignments
        .set(ConsumerId::new(80), expressions[1])
        .unwrap();
    let assignment_result = resolve(&graph, &atoms, &requirements, &changed_assignments);
    assert_ne!(assignment_result.0, baseline.0);
    assert_ne!(assignment_result.1, baseline.1);
    assert_eq!(
        assignment_result
            .1
            .get(expressions[0], AbsoluteBits::new(20)),
        baseline.1.get(expressions[0], AbsoluteBits::new(20))
    );
    assert_eq!(
        assignment_result.2.get(ConsumerId::new(20)),
        baseline.2.get(ConsumerId::new(20))
    );
    assert_ne!(
        assignment_result.2.get(ConsumerId::new(80)),
        baseline.2.get(ConsumerId::new(80))
    );
    assert_eq!(graph, original_graph);
    assert_eq!(atoms, original_atoms);
    assert_eq!(requirements, original_requirements);
    assert_ne!(changed_assignments, original_assignments);
}
