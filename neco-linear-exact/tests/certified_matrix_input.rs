use neco_expr::{
    allocate_exact_numeric, assemble_exact_computation_product, decide_exact_properties,
    normalize_exact_expressions, read_exact_numeric_inputs, resolve_certified_f64, AbsoluteBits,
    AtomId, AtomStore, ConsumerId, ExactAllocationInput, ExactDecisionRequest,
    ExactExpressionRequirement, ExactOperation, ExactValue, ExprGraph, ExprId, ExprNode,
    NecoFailure, NecoImplementationSource, NecoObservedCapability, OwnerExpressionSet, OwnerId,
};
use neco_formsum::FormSum;
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

fn exact_integer(value: i32) -> ExactValue {
    let mut sum = FormSum::zero();
    for _ in 0..value.unsigned_abs() {
        sum = sum.add(&FormSum::one().expect("one")).expect("sum");
    }
    if value < 0 {
        ExactValue::FormSum(FormSum::zero().sub(&sum).expect("negative sum"))
    } else {
        ExactValue::FormSum(sum)
    }
}

fn product(values: [i32; 4], precision: u32) -> neco_expr::ExactComputationProduct {
    let mut graph = ExprGraph::new();
    let mut atoms = AtomStore::new();
    for (index, value) in values.into_iter().enumerate() {
        let expression = graph
            .push(ExprNode::Atom(AtomId::new(index as u32)))
            .expect("expression");
        assert_eq!(expression, ExprId::new(index as u32));
        atoms
            .insert(AtomId::new(index as u32), exact_integer(value))
            .expect("atom");
    }
    let requirement = ExactExpressionRequirement::new(
        ConsumerId::new(11),
        &[
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
        ],
        ExactDecisionRequest::Zero(ExprId::new(0)),
        AbsoluteBits::new(precision),
    )
    .expect("requirement");
    let first_owner = OwnerExpressionSet::new(OwnerId::new(1), graph, atoms, vec![requirement])
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

fn input(
    values: [i32; 4],
    precision: u32,
) -> (neco_expr::ExactComputationProduct, CertifiedMatrixInput) {
    let product = product(values, precision);
    let projection = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(11))
        .expect("projection");
    let matrix =
        DenseMatrix::from_row_major(Shape::new(2, 2), values.map(|value| value as f64).to_vec())
            .expect("matrix");
    let certified = CertifiedMatrixInput::from_product_projection(
        &product,
        projection,
        matrix,
        vec![
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
        ],
        OwnerId::new(1),
        ConsumerId::new(11),
    )
    .expect("certified matrix");
    (product, certified)
}

#[test]
fn certified_matrix_input_accepts_row_major_projection() {
    let (_, input) = input([1, 0, 0, 1], 30);
    let entry = input.entry(1, 0).expect("entry");
    assert_eq!(entry.row(), 1);
    assert_eq!(entry.column(), 0);
    assert_eq!(entry.expression(), ExprId::new(2));
    assert_eq!(entry.consumer_id(), ConsumerId::new(11));
    assert_eq!(entry.value(), 0.0);
    assert_eq!(
        input.absolute_error_bound(),
        &neco_bigint::Dyadic::from_f64_exact(0.0).unwrap()
    );
}

#[test]
fn matrix_entry_wrong_expr_is_rejected() {
    let (product, _) = input([1, 0, 0, 1], 30);
    let projection = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(11))
        .unwrap();
    let result = CertifiedMatrixInput::from_product_projection(
        &product,
        projection,
        DenseMatrix::from_row_major(Shape::new(2, 2), vec![1.0, 0.0, 0.0, 1.0]).unwrap(),
        vec![
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(3),
            ExprId::new(2),
        ],
        OwnerId::new(1),
        ConsumerId::new(11),
    );
    assert!(matches!(
        result,
        Err(NecoFailure::MatrixInputMismatch { location, payload })
            if location.row() == Some(1)
                && location.column() == Some(0)
                && payload.expected_expr() == Some(ExprId::new(3))
                && payload.actual_expr() == Some(ExprId::new(2))
    ));
}

#[test]
fn matrix_entry_numeric_value_mismatch_is_rejected() {
    let (product, _) = input([1, 0, 0, 1], 30);
    let projection = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(11))
        .unwrap();
    let result = CertifiedMatrixInput::from_product_projection(
        &product,
        projection,
        DenseMatrix::from_row_major(
            Shape::new(2, 2),
            vec![1.0, f64::from_bits(0.0_f64.to_bits() ^ 1), 0.0, 1.0],
        )
        .unwrap(),
        vec![
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
        ],
        OwnerId::new(1),
        ConsumerId::new(11),
    );
    assert!(matches!(
        result,
        Err(NecoFailure::MatrixInputMismatch { payload, .. })
            if payload.field() == neco_expr::MatrixInputMismatchField::NumericValue
    ));
}

#[test]
fn matrix_entry_wrong_consumer_is_rejected() {
    let (product, _) = input([1, 0, 0, 1], 30);
    let projection = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(11))
        .unwrap();
    let result = CertifiedMatrixInput::from_product_projection(
        &product,
        projection,
        DenseMatrix::from_row_major(Shape::new(2, 2), vec![1.0, 0.0, 0.0, 1.0]).unwrap(),
        vec![
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
        ],
        OwnerId::new(1),
        ConsumerId::new(12),
    );
    assert!(matches!(
        result,
        Err(NecoFailure::ProvenanceMismatch { location, payload })
            if payload.field() == neco_expr::ProvenanceMismatchField::Consumer
                && location.consumer() == Some(ConsumerId::new(12))
    ));
}

#[test]
fn matrix_entry_wrong_owner_is_rejected() {
    let (product, _) = input([1, 0, 0, 1], 30);
    let projection = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(11))
        .unwrap();
    let result = CertifiedMatrixInput::from_product_projection(
        &product,
        projection,
        DenseMatrix::from_row_major(Shape::new(2, 2), vec![1.0, 0.0, 0.0, 1.0]).unwrap(),
        vec![
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
        ],
        OwnerId::new(2),
        ConsumerId::new(11),
    );
    assert!(matches!(
        result,
        Err(NecoFailure::ProvenanceMismatch { payload, .. })
            if payload.field() == neco_expr::ProvenanceMismatchField::Owner
    ));
}

#[test]
fn matrix_entry_wrong_precision_is_rejected() {
    let (product, _) = input([1, 0, 0, 1], 24);
    let (other, _) = input([1, 0, 0, 1], 40);
    let projection = other
        .certified_descent(OwnerId::new(1), ConsumerId::new(11))
        .unwrap();
    let result = CertifiedMatrixInput::from_product_projection(
        &product,
        projection,
        DenseMatrix::from_row_major(Shape::new(2, 2), vec![1.0, 0.0, 0.0, 1.0]).unwrap(),
        vec![
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
        ],
        OwnerId::new(1),
        ConsumerId::new(11),
    );
    assert!(matches!(
        result,
        Err(NecoFailure::ProvenanceMismatch { payload, .. })
            if payload.field() == neco_expr::ProvenanceMismatchField::Precision
    ));
}

#[test]
fn matrix_entry_projection_from_another_product_is_rejected() {
    let (product, _) = input([1, 0, 0, 1], 30);
    let (other, _) = input([1, 0, 0, 1], 30);
    assert_ne!(product.product_identity(), other.product_identity());
    let projection = other
        .certified_descent(OwnerId::new(1), ConsumerId::new(11))
        .unwrap();
    let result = CertifiedMatrixInput::from_product_projection(
        &product,
        projection,
        DenseMatrix::from_row_major(Shape::new(2, 2), vec![1.0, 0.0, 0.0, 1.0]).unwrap(),
        vec![
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
        ],
        OwnerId::new(1),
        ConsumerId::new(11),
    );
    assert!(matches!(
        result,
        Err(NecoFailure::ProvenanceMismatch { location, payload })
            if payload.field() == neco_expr::ProvenanceMismatchField::Product
                && payload.expected().product_identity().is_some()
                && payload.actual().product_identity().is_some()
                && location.row() == Some(0)
                && location.column() == Some(0)
    ));
}

#[test]
fn matrix_entry_missing_mass_reference_is_rejected() {
    let (_, stiffness) = input([1, 0, 0, 1], 30);
    assert!(matches!(
        CertifiedGeneralizedEigenInput::new(stiffness, None),
        Err(NecoFailure::MissingMatrixInput { location })
            if location.matrix_role() == Some(neco_expr::MatrixInputRole::MassNormalization)
    ));
}

#[test]
fn matrix_input_rejects_missing_duplicate_and_extra_expression() {
    let (product, _) = input([1, 0, 0, 1], 30);
    let projection = product
        .certified_descent(OwnerId::new(1), ConsumerId::new(11))
        .unwrap();
    let matrix =
        || DenseMatrix::from_row_major(Shape::new(2, 2), vec![1.0, 0.0, 0.0, 1.0]).unwrap();
    let missing = CertifiedMatrixInput::from_product_projection(
        &product,
        projection,
        matrix(),
        vec![ExprId::new(0), ExprId::new(1), ExprId::new(2)],
        OwnerId::new(1),
        ConsumerId::new(11),
    );
    assert!(matches!(
        missing,
        Err(NecoFailure::MatrixInputMismatch { payload, .. })
            if payload.field() == neco_expr::MatrixInputMismatchField::MissingExpression
    ));
    let duplicate = CertifiedMatrixInput::from_product_projection(
        &product,
        projection,
        matrix(),
        vec![
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(2),
        ],
        OwnerId::new(1),
        ConsumerId::new(11),
    );
    assert!(matches!(
        duplicate,
        Err(NecoFailure::MatrixInputMismatch { payload, .. })
            if payload.field() == neco_expr::MatrixInputMismatchField::DuplicateExpression
    ));
    let extra = CertifiedMatrixInput::from_product_projection(
        &product,
        projection,
        matrix(),
        vec![
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
            ExprId::new(4),
        ],
        OwnerId::new(1),
        ConsumerId::new(11),
    );
    assert!(matches!(
        extra,
        Err(NecoFailure::MatrixInputMismatch { payload, .. })
            if payload.field() == neco_expr::MatrixInputMismatchField::ExtraExpression
    ));
}

#[test]
fn mass_normalization_reference_requires_same_product_and_shape() {
    let (_, stiffness) = input([1, 0, 0, 1], 30);
    let (_, mass) = input([1, 0, 0, 1], 30);
    let mass = MassNormalizationReference::new(mass, MassNormalizationMethod::Identity).unwrap();
    assert!(matches!(
        CertifiedGeneralizedEigenInput::new(stiffness, mass),
        Err(NecoFailure::ProvenanceMismatch { payload, .. })
            if payload.field() == neco_expr::ProvenanceMismatchField::Product
    ));
}

#[test]
fn certified_matrix_input_keeps_the_product_identity_opaque_and_per_product() {
    let (first, first_input) = input([1, 0, 0, 1], 30);
    let (second, second_input) = input([1, 0, 0, 1], 30);
    assert_ne!(first.product_identity(), second.product_identity());
    assert_eq!(first_input.product_identity(), first.product_identity());
    assert_eq!(second_input.product_identity(), second.product_identity());
    assert_eq!(
        first_input.projection().product_identity(),
        first.product_identity()
    );
    assert_eq!(
        second_input.projection().product_identity(),
        second.product_identity()
    );
}
