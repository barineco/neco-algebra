use neco_bigint::{BigInt, Dyadic};
use neco_expr::{
    AbsoluteBits, ConsumerId, ExprId, FailureLocation, FailureOperation, MatrixInputMismatchField,
    MatrixInputMismatchPayload, MatrixInputRole, NecoFailure, OwnerId, PrecisionFailurePayload,
    ProvenanceMismatchField, ProvenanceMismatchPayload, ProvenanceRecord, StorageFailurePayload,
    StorageResource, UnsupportedFailurePayload,
};

const STORAGE_GRAPH_ID_EXHAUSTED: &str = r#"StorageFailure { location: FailureLocation { operation: Allocate, consumer: None, expr: None, atom: None, decision: None, matrix_role: None, row: None, column: None }, payload: GraphIdExhausted }"#;
const STORAGE_DUPLICATE_ID: &str = r#"StorageFailure { location: FailureLocation { operation: Allocate, consumer: Some(ConsumerId(3)), expr: None, atom: None, decision: None, matrix_role: None, row: None, column: None }, payload: DuplicateId { resource: ProductEntries, id: 7 } }"#;
const UNSUPPORTED_REQUIRED_OPERATION: &str = r#"UnsupportedRequiredOperation { location: FailureLocation { operation: Read, consumer: None, expr: None, atom: None, decision: None, matrix_role: None, row: None, column: None }, payload: RequiredOperation("resolve-certified-f64") }"#;
const PRECISION_EXHAUSTION: &str = r#"PrecisionExhaustion { location: FailureLocation { operation: Resolve, consumer: None, expr: None, atom: None, decision: None, matrix_role: None, row: None, column: None }, requested: AbsoluteBits(8), payload: EnclosureWidth { actual_width: Dyadic { integer: BigInt { sign: Positive, magnitude: BigUint { limbs: [1] } }, exponent: 0 } } }"#;
const PROVENANCE_MISMATCH: &str = r#"ProvenanceMismatch { location: FailureLocation { operation: Assemble, consumer: Some(ConsumerId(1)), expr: None, atom: None, decision: None, matrix_role: None, row: None, column: None }, payload: ProvenanceMismatchPayload { field: Owner, expected: ProvenanceRecord { owner: Some(OwnerId(1)), consumer_id: Some(ConsumerId(1)), expr_id: Some(ExprId(2)), precision: Some(AbsoluteBits(8)), revision_identity: Some("rev"), public_api_identity: Some("api"), product_identity: None }, actual: ProvenanceRecord { owner: Some(OwnerId(2)), consumer_id: Some(ConsumerId(1)), expr_id: Some(ExprId(2)), precision: Some(AbsoluteBits(8)), revision_identity: Some("rev"), public_api_identity: Some("api"), product_identity: None } } }"#;
const MATRIX_INPUT_MISMATCH: &str = r#"MatrixInputMismatch { location: FailureLocation { operation: Read, consumer: Some(ConsumerId(1)), expr: None, atom: None, decision: None, matrix_role: Some(Stiffness), row: Some(1), column: Some(0) }, payload: MatrixInputMismatchPayload { field: NumericValue, row: Some(1), column: Some(0), expected: ProvenanceRecord { owner: Some(OwnerId(1)), consumer_id: Some(ConsumerId(1)), expr_id: Some(ExprId(2)), precision: Some(AbsoluteBits(8)), revision_identity: Some("rev"), public_api_identity: Some("api"), product_identity: None }, actual: ProvenanceRecord { owner: Some(OwnerId(2)), consumer_id: Some(ConsumerId(1)), expr_id: Some(ExprId(2)), precision: Some(AbsoluteBits(8)), revision_identity: Some("rev"), public_api_identity: Some("api"), product_identity: None }, expected_product: None, actual_product: None, expected_expr: Some(ExprId(3)), actual_expr: Some(ExprId(4)), expected_value_bits: Some(4607182418800017408), actual_value_bits: Some(4611686018427387904) } }"#;

fn provenance(owner: OwnerId) -> ProvenanceRecord {
    ProvenanceRecord::new(
        Some(owner),
        Some(ConsumerId::new(1)),
        Some(ExprId::new(2)),
        Some(AbsoluteBits::new(8)),
        Some("rev"),
        Some("api"),
    )
}

#[test]
fn neco_failure_debug_text_keeps_the_unboxed_form() {
    let cases = [
        (
            NecoFailure::StorageFailure {
                location: FailureLocation::operation(FailureOperation::Allocate),
                payload: Box::new(StorageFailurePayload::GraphIdExhausted),
            },
            STORAGE_GRAPH_ID_EXHAUSTED,
        ),
        (
            NecoFailure::StorageFailure {
                location: FailureLocation::operation(FailureOperation::Allocate)
                    .with_consumer(ConsumerId::new(3)),
                payload: Box::new(StorageFailurePayload::DuplicateId {
                    resource: StorageResource::ProductEntries,
                    id: 7,
                }),
            },
            STORAGE_DUPLICATE_ID,
        ),
        (
            NecoFailure::UnsupportedRequiredOperation {
                location: FailureLocation::operation(FailureOperation::Read),
                payload: Box::new(UnsupportedFailurePayload::RequiredOperation(
                    "resolve-certified-f64",
                )),
            },
            UNSUPPORTED_REQUIRED_OPERATION,
        ),
        (
            NecoFailure::PrecisionExhaustion {
                location: FailureLocation::operation(FailureOperation::Resolve),
                requested: AbsoluteBits::new(8),
                payload: Box::new(PrecisionFailurePayload::EnclosureWidth {
                    actual_width: Dyadic::new(BigInt::try_from(1).unwrap(), 0),
                }),
            },
            PRECISION_EXHAUSTION,
        ),
        (
            NecoFailure::ProvenanceMismatch {
                location: FailureLocation::operation(FailureOperation::Assemble)
                    .with_consumer(ConsumerId::new(1)),
                payload: Box::new(ProvenanceMismatchPayload::new(
                    ProvenanceMismatchField::Owner,
                    provenance(OwnerId::new(1)),
                    provenance(OwnerId::new(2)),
                )),
            },
            PROVENANCE_MISMATCH,
        ),
        (
            NecoFailure::MatrixInputMismatch {
                location: FailureLocation::operation(FailureOperation::Read)
                    .with_consumer(ConsumerId::new(1))
                    .with_matrix_role(MatrixInputRole::Stiffness)
                    .with_matrix_location(1, 0),
                payload: Box::new(
                    MatrixInputMismatchPayload::new(
                        MatrixInputMismatchField::NumericValue,
                        provenance(OwnerId::new(1)),
                        provenance(OwnerId::new(2)),
                    )
                    .with_location(1, 0)
                    .with_expressions(Some(ExprId::new(3)), Some(ExprId::new(4)))
                    .with_value_bits(Some(0x3ff0_0000_0000_0000), Some(0x4000_0000_0000_0000)),
                ),
            },
            MATRIX_INPUT_MISMATCH,
        ),
    ];
    for (failure, expected) in cases {
        assert_eq!(format!("{failure:?}"), expected);
    }
}
