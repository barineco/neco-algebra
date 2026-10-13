use neco_algnum::RealAlgebraic;
use neco_bigint::{BigInt, BigUint, RawRational, Sign};
use neco_eigensolve::EigensolveConfig;
use neco_expr::{
    allocate_exact_numeric, assemble_exact_computation_product, decide_exact_properties,
    normalize_exact_expressions, read_exact_numeric_inputs, resolve_certified_f64, AbsoluteBits,
    AtomId, AtomStore, ConsumerId, ExactAllocationInput, ExactDecisionRequest,
    ExactExpressionRequirement, ExactOperation, ExactValue, ExprGraph, ExprId, ExprNode,
    NecoImplementationSource, NecoObservedCapability, OwnerExpressionSet, OwnerId,
};
use neco_formsum::{RawFormSum, RawTerm};
use neco_linear_dense::DenseMatrix;
use neco_linear_exact::CertifiedMatrixInput;
use neco_linear_types::Shape;
use neco_monomial::RawMonomial;

pub const OPERATIONS: [ExactOperation; 8] = [
    ExactOperation::NormalizeMonomial,
    ExactOperation::NormalizeFormSum,
    ExactOperation::NormalizeAlgebraic,
    ExactOperation::DecideZero,
    ExactOperation::DecideEquality,
    ExactOperation::DecideSign,
    ExactOperation::DecideDegeneracy,
    ExactOperation::ResolveCertifiedF64,
];

pub enum Entry {
    Rational(i64, i64),
    Sqrt2,
}

pub fn rational_value(numerator: i64, denominator: i64) -> ExactValue {
    let sign = if numerator < 0 {
        Sign::Negative
    } else {
        Sign::Positive
    };
    let numerator_magnitude = BigUint::try_from(numerator.unsigned_abs()).expect("magnitude");
    let numerator_bigint = BigInt::from_sign_magnitude(sign, numerator_magnitude);
    let denominator_magnitude = BigUint::try_from(denominator as u64).expect("denominator");
    let coefficient = RawRational::new(numerator_bigint, denominator_magnitude);
    let term = RawTerm::new(coefficient, RawMonomial::positive(Vec::new()));
    let form_sum = RawFormSum::new(vec![term]).normalize().expect("normalize");
    ExactValue::FormSum(form_sum)
}

fn sqrt2_value() -> ExactValue {
    let two = RealAlgebraic::from_integer(BigInt::try_from(2_i64).expect("two"))
        .expect("two as algebraic");
    ExactValue::Algebraic(two.nth_root(2).expect("square root"))
}

fn entry_value(entry: &Entry) -> ExactValue {
    match entry {
        Entry::Rational(numerator, denominator) => rational_value(*numerator, *denominator),
        Entry::Sqrt2 => sqrt2_value(),
    }
}

fn entry_f64(entry: &Entry) -> f64 {
    match entry {
        Entry::Rational(numerator, denominator) => *numerator as f64 / *denominator as f64,
        Entry::Sqrt2 => std::f64::consts::SQRT_2,
    }
}

pub fn rational(numerator: i64) -> Entry {
    Entry::Rational(numerator, 1)
}

pub fn certified_pair_with_precision(
    dimension: usize,
    stiffness_entries: Vec<Entry>,
    mass_entries: Vec<Entry>,
    bits: u32,
) -> (CertifiedMatrixInput, CertifiedMatrixInput) {
    let count = dimension * dimension;
    assert_eq!(stiffness_entries.len(), count);
    assert_eq!(mass_entries.len(), count);

    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    for (index, entry) in stiffness_entries
        .iter()
        .chain(mass_entries.iter())
        .enumerate()
    {
        let index = index as u32;
        graph
            .push(ExprNode::Atom(AtomId::new(index)))
            .expect("expression");
        atoms
            .insert(AtomId::new(index), entry_value(entry))
            .expect("atom");
    }

    let stiffness_ids: Vec<ExprId> = (0..count).map(|index| ExprId::new(index as u32)).collect();
    let mass_ids: Vec<ExprId> = (count..2 * count)
        .map(|index| ExprId::new(index as u32))
        .collect();

    let stiffness_requirement = ExactExpressionRequirement::new(
        ConsumerId::new(11),
        &stiffness_ids,
        ExactDecisionRequest::Zero(stiffness_ids[0]),
        AbsoluteBits::new(bits),
    )
    .expect("stiffness requirement");
    let mass_requirement = ExactExpressionRequirement::new(
        ConsumerId::new(12),
        &mass_ids,
        ExactDecisionRequest::Zero(mass_ids[0]),
        AbsoluteBits::new(bits),
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

    let shape = Shape::new(dimension, dimension);
    let stiffness_matrix =
        DenseMatrix::from_row_major(shape, stiffness_entries.iter().map(entry_f64).collect())
            .expect("stiffness matrix");
    let mass_matrix =
        DenseMatrix::from_row_major(shape, mass_entries.iter().map(entry_f64).collect())
            .expect("mass matrix");

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

    (stiffness, mass)
}

pub fn certified_pair(
    dimension: usize,
    stiffness_values: Vec<(i64, i64)>,
    mass_values: Vec<(i64, i64)>,
) -> (CertifiedMatrixInput, CertifiedMatrixInput) {
    let to_entries = |values: Vec<(i64, i64)>| {
        values
            .into_iter()
            .map(|(numerator, denominator)| Entry::Rational(numerator, denominator))
            .collect()
    };
    certified_pair_with_precision(
        dimension,
        to_entries(stiffness_values),
        to_entries(mass_values),
        30,
    )
}

pub fn config(requested_modes: usize, max_iterations: usize) -> EigensolveConfig {
    EigensolveConfig::new(requested_modes, 1.0e-12, 1.0e-12, max_iterations).expect("config")
}
