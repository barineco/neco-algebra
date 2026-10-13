use alloc::boxed::Box;
use core::fmt;

use neco_algnum::AlgnumError;
use neco_bigint::{BigintError, Dyadic};
use neco_formsum::FormSumErrorKind;
use neco_monomial::MonomialErrorKind;

use crate::product::ComputationProductId;
use crate::{AbsoluteBits, AtomId, ConsumerId, ExprId, OwnerId, StorageResource};

#[derive(Debug, Eq, PartialEq)]
pub enum StorageError {
    CapacityOverflow {
        resource: StorageResource,
    },
    AllocationFailure {
        resource: StorageResource,
        requested_elements: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GraphError {
    IdExhausted,
    InvalidChildId { child: ExprId, next: ExprId },
    Node(EvalError),
    Storage(StorageError),
}

#[derive(Debug, Eq, PartialEq)]
pub enum InsertError {
    DuplicateId { resource: StorageResource, id: u32 },
    Value(EvalError),
    Storage(StorageError),
}

#[derive(Debug, Eq, PartialEq)]
pub enum EvalError {
    DivisionByZero,
    UndefinedZeroPower,
    ZeroToNegativePower,
    EvenRootOfNegative,
    Bigint(BigintError),
    Monomial(MonomialErrorKind),
    FormSum(FormSumErrorKind),
    Algnum(AlgnumError),
}

#[derive(Debug, Eq, PartialEq)]
pub enum ResolveError {
    MissingAssignment { consumer: ConsumerId },
    UnknownExprId { consumer: ConsumerId, expr: ExprId },
    UnknownAtomId { expr: ExprId, atom: AtomId },
    FloatOutOfRange { consumer: ConsumerId, expr: ExprId },
    Evaluation(EvalError),
    Bigint(BigintError),
    Algnum(AlgnumError),
    Storage(StorageError),
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityOverflow { resource } => {
                write!(formatter, "capacity overflow for {resource:?}")
            }
            Self::AllocationFailure {
                resource,
                requested_elements,
            } => write!(
                formatter,
                "allocation failed for {resource:?}: requested {requested_elements} elements"
            ),
        }
    }
}

impl fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdExhausted => formatter.write_str("expression graph ID space is exhausted"),
            Self::InvalidChildId { child, next } => write!(
                formatter,
                "expression child {} is not earlier than next expression {}",
                child.get(),
                next.get()
            ),
            Self::Node(error) => write!(formatter, "expression node clone failed: {error}"),
            Self::Storage(error) => error.fmt(formatter),
        }
    }
}

impl fmt::Display for InsertError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId { resource, id } => {
                write!(formatter, "duplicate ID {id} for {resource:?}")
            }
            Self::Value(error) => write!(formatter, "stored value clone failed: {error}"),
            Self::Storage(error) => error.fmt(formatter),
        }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DivisionByZero => formatter.write_str("division by zero"),
            Self::UndefinedZeroPower => formatter.write_str("zero raised to the zero power"),
            Self::ZeroToNegativePower => formatter.write_str("zero raised to a negative power"),
            Self::EvenRootOfNegative => formatter.write_str("even root of a negative value"),
            Self::Bigint(error) => error.fmt(formatter),
            Self::Monomial(error) => error.fmt(formatter),
            Self::FormSum(error) => error.fmt(formatter),
            Self::Algnum(error) => error.fmt(formatter),
        }
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAssignment { consumer } => {
                write!(formatter, "consumer {} has no assignment", consumer.get())
            }
            Self::UnknownExprId { consumer, expr } => write!(
                formatter,
                "consumer {} refers to unknown expression {}",
                consumer.get(),
                expr.get()
            ),
            Self::UnknownAtomId { expr, atom } => write!(
                formatter,
                "expression {} refers to unknown atom {}",
                expr.get(),
                atom.get()
            ),
            Self::FloatOutOfRange { consumer, expr } => write!(
                formatter,
                "expression {} for consumer {} is outside finite f64 range",
                expr.get(),
                consumer.get()
            ),
            Self::Evaluation(error) => error.fmt(formatter),
            Self::Bigint(error) => error.fmt(formatter),
            Self::Algnum(error) => error.fmt(formatter),
            Self::Storage(error) => error.fmt(formatter),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StorageError {}

#[cfg(feature = "std")]
impl std::error::Error for GraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Node(error) => Some(error),
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InsertError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Value(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::DuplicateId { .. } => None,
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EvalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Bigint(error) => Some(error),
            Self::Monomial(error) => Some(error),
            Self::FormSum(error) => Some(error),
            Self::Algnum(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Evaluation(error) => Some(error),
            Self::Bigint(error) => Some(error),
            Self::Algnum(error) => Some(error),
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

impl From<BigintError> for EvalError {
    fn from(error: BigintError) -> Self {
        Self::Bigint(error)
    }
}

impl From<MonomialErrorKind> for EvalError {
    fn from(error: MonomialErrorKind) -> Self {
        Self::Monomial(error)
    }
}

impl From<FormSumErrorKind> for EvalError {
    fn from(error: FormSumErrorKind) -> Self {
        Self::FormSum(error)
    }
}

impl From<AlgnumError> for EvalError {
    fn from(error: AlgnumError) -> Self {
        Self::Algnum(error)
    }
}

impl From<StorageError> for InsertError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

pub(crate) fn try_clone_eval_error(error: &EvalError) -> Result<EvalError, EvalError> {
    match error {
        EvalError::DivisionByZero => Ok(EvalError::DivisionByZero),
        EvalError::UndefinedZeroPower => Ok(EvalError::UndefinedZeroPower),
        EvalError::ZeroToNegativePower => Ok(EvalError::ZeroToNegativePower),
        EvalError::EvenRootOfNegative => Ok(EvalError::EvenRootOfNegative),
        EvalError::Bigint(error) => Ok(EvalError::Bigint(try_clone_bigint_error(error)?)),
        EvalError::Monomial(error) => error
            .try_clone()
            .map(EvalError::Monomial)
            .map_err(EvalError::Monomial),
        EvalError::FormSum(error) => Ok(EvalError::FormSum(try_clone_formsum_error(error)?)),
        EvalError::Algnum(error) => Ok(EvalError::Algnum(try_clone_algnum_error(error)?)),
    }
}

fn try_clone_bigint_error(error: &BigintError) -> Result<BigintError, EvalError> {
    Ok(match error {
        BigintError::CapacityOverflow => BigintError::CapacityOverflow,
        BigintError::AllocationFailure { requested_limbs } => BigintError::AllocationFailure {
            requested_limbs: *requested_limbs,
        },
        BigintError::UnsignedUnderflow => BigintError::UnsignedUnderflow,
        BigintError::DivisionByZero => BigintError::DivisionByZero,
        BigintError::NonExactDivision => BigintError::NonExactDivision,
        BigintError::ZeroDenominator => BigintError::ZeroDenominator,
        BigintError::NonFiniteFloat => BigintError::NonFiniteFloat,
        BigintError::FloatOutOfRange => BigintError::FloatOutOfRange,
        BigintError::InvalidInterval => BigintError::InvalidInterval,
        BigintError::ExponentOverflow { required, maximum } => BigintError::ExponentOverflow {
            required: required.try_clone().map_err(EvalError::Bigint)?,
            maximum: *maximum,
        },
    })
}

fn try_clone_formsum_error(error: &FormSumErrorKind) -> Result<FormSumErrorKind, EvalError> {
    Ok(match error {
        FormSumErrorKind::DivisionByZero => FormSumErrorKind::DivisionByZero,
        FormSumErrorKind::DimensionOverflow {
            resource,
            required,
            maximum,
        } => FormSumErrorKind::DimensionOverflow {
            resource: *resource,
            required: required.try_clone().map_err(EvalError::Bigint)?,
            maximum: maximum.try_clone().map_err(EvalError::Bigint)?,
        },
        FormSumErrorKind::AllocationFailure {
            resource,
            requested,
        } => FormSumErrorKind::AllocationFailure {
            resource: *resource,
            requested: *requested,
        },
        FormSumErrorKind::Bigint(error) => FormSumErrorKind::Bigint(try_clone_bigint_error(error)?),
        FormSumErrorKind::Monomial(error) => {
            FormSumErrorKind::Monomial(error.try_clone().map_err(EvalError::Monomial)?)
        }
    })
}

fn try_clone_algnum_error(error: &AlgnumError) -> Result<AlgnumError, EvalError> {
    Ok(match error {
        AlgnumError::ZeroPolynomial => AlgnumError::ZeroPolynomial,
        AlgnumError::InvalidIsolation => AlgnumError::InvalidIsolation,
        AlgnumError::NoTargetRoot => AlgnumError::NoTargetRoot,
        AlgnumError::MultipleTargetRoots => AlgnumError::MultipleTargetRoots,
        AlgnumError::DivisionByZero => AlgnumError::DivisionByZero,
        AlgnumError::UndefinedZeroPower => AlgnumError::UndefinedZeroPower,
        AlgnumError::ZeroToNegativePower => AlgnumError::ZeroToNegativePower,
        AlgnumError::ZeroRootDegree => AlgnumError::ZeroRootDegree,
        AlgnumError::EvenRootOfNegative => AlgnumError::EvenRootOfNegative,
        AlgnumError::RepresentationLimit {
            resource,
            required,
            maximum,
        } => AlgnumError::RepresentationLimit {
            resource: *resource,
            required: required.try_clone().map_err(EvalError::Bigint)?,
            maximum: maximum.try_clone().map_err(EvalError::Bigint)?,
        },
        AlgnumError::AllocationLimit {
            resource,
            required,
            maximum,
        } => AlgnumError::AllocationLimit {
            resource: *resource,
            required: required.try_clone().map_err(EvalError::Bigint)?,
            maximum: maximum.try_clone().map_err(EvalError::Bigint)?,
        },
        AlgnumError::AllocationFailure {
            resource,
            requested,
        } => AlgnumError::AllocationFailure {
            resource: *resource,
            requested: *requested,
        },
        AlgnumError::Bigint(error) => AlgnumError::Bigint(try_clone_bigint_error(error)?),
        AlgnumError::FormSum(error) => AlgnumError::FormSum(try_clone_formsum_error(error)?),
    })
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::error::Error;

    use neco_bigint::BigintError;
    use neco_formsum::FormSumErrorKind;
    use neco_monomial::MonomialErrorKind;

    use crate::{EvalError, GraphError, InsertError, ResolveError, StorageError, StorageResource};

    #[test]
    fn error_sources_are_exactly_the_owned_lower_failures() {
        let bigint = EvalError::Bigint(BigintError::DivisionByZero);
        assert!(core::ptr::eq(
            bigint
                .source()
                .unwrap()
                .downcast_ref::<BigintError>()
                .unwrap(),
            match &bigint {
                EvalError::Bigint(error) => error,
                _ => unreachable!(),
            }
        ));
        let monomial = EvalError::Monomial(MonomialErrorKind::DivisionByZero);
        assert!(monomial.source().unwrap().is::<MonomialErrorKind>());
        let form_sum = EvalError::FormSum(FormSumErrorKind::DivisionByZero);
        assert!(form_sum.source().unwrap().is::<FormSumErrorKind>());

        let graph = GraphError::Node(EvalError::DivisionByZero);
        assert!(graph.source().unwrap().is::<EvalError>());
        let insert = InsertError::Value(EvalError::DivisionByZero);
        assert!(insert.source().unwrap().is::<EvalError>());
        let resolve = ResolveError::Evaluation(EvalError::DivisionByZero);
        assert!(resolve.source().unwrap().is::<EvalError>());
        let storage = StorageError::CapacityOverflow {
            resource: StorageResource::GraphNodes,
        };
        let graph_storage = GraphError::Storage(storage);
        assert!(graph_storage.source().unwrap().is::<StorageError>());

        assert!(EvalError::DivisionByZero.source().is_none());
        assert!(GraphError::IdExhausted.source().is_none());
        assert!(InsertError::DuplicateId {
            resource: StorageResource::AtomEntries,
            id: 1,
        }
        .source()
        .is_none());
    }

    #[test]
    fn lower_error_conversions_preserve_variants_and_payloads() {
        assert_eq!(
            EvalError::from(MonomialErrorKind::AllocationFailure {
                requested_elements: 23,
            }),
            EvalError::Monomial(MonomialErrorKind::AllocationFailure {
                requested_elements: 23,
            })
        );
        assert_eq!(
            EvalError::from(FormSumErrorKind::DivisionByZero),
            EvalError::FormSum(FormSumErrorKind::DivisionByZero)
        );
        assert_eq!(
            EvalError::from(BigintError::AllocationFailure { requested_limbs: 5 }),
            EvalError::Bigint(BigintError::AllocationFailure { requested_limbs: 5 })
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FailureOperation {
    Read,
    Allocate,
    Normalize,
    Decide,
    Resolve,
    Assemble,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MatrixInputRole {
    Matrix,
    Stiffness,
    Mass,
    MassNormalization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureLocation {
    operation: FailureOperation,
    consumer: Option<ConsumerId>,
    expr: Option<ExprId>,
    atom: Option<AtomId>,
    decision: Option<u32>,
    matrix_role: Option<MatrixInputRole>,
    row: Option<usize>,
    column: Option<usize>,
}

impl FailureLocation {
    pub const fn operation(operation: FailureOperation) -> Self {
        Self {
            operation,
            consumer: None,
            expr: None,
            atom: None,
            decision: None,
            matrix_role: None,
            row: None,
            column: None,
        }
    }

    pub const fn with_consumer(mut self, consumer: ConsumerId) -> Self {
        self.consumer = Some(consumer);
        self
    }

    pub const fn with_expr(mut self, expr: ExprId) -> Self {
        self.expr = Some(expr);
        self
    }

    pub const fn with_atom(mut self, atom: AtomId) -> Self {
        self.atom = Some(atom);
        self
    }

    pub const fn with_decision(mut self, decision: u32) -> Self {
        self.decision = Some(decision);
        self
    }

    pub const fn with_matrix_role(mut self, role: MatrixInputRole) -> Self {
        self.matrix_role = Some(role);
        self
    }

    pub const fn with_matrix_location(mut self, row: usize, column: usize) -> Self {
        self.row = Some(row);
        self.column = Some(column);
        self
    }

    pub const fn operation_kind(&self) -> FailureOperation {
        self.operation
    }

    pub const fn consumer(&self) -> Option<ConsumerId> {
        self.consumer
    }

    pub const fn expr(&self) -> Option<ExprId> {
        self.expr
    }

    pub const fn atom(&self) -> Option<AtomId> {
        self.atom
    }

    pub const fn decision(&self) -> Option<u32> {
        self.decision
    }

    pub const fn matrix_role(&self) -> Option<MatrixInputRole> {
        self.matrix_role
    }

    pub const fn role(&self) -> Option<MatrixInputRole> {
        self.matrix_role()
    }

    pub const fn row(&self) -> Option<usize> {
        self.row
    }

    pub const fn column(&self) -> Option<usize> {
        self.column
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvenanceRecord {
    owner: Option<OwnerId>,
    consumer_id: Option<ConsumerId>,
    expr_id: Option<ExprId>,
    precision: Option<AbsoluteBits>,
    revision_identity: Option<&'static str>,
    public_api_identity: Option<&'static str>,
    product_identity: Option<ComputationProductId>,
}

impl ProvenanceRecord {
    pub const fn new(
        owner: Option<OwnerId>,
        consumer_id: Option<ConsumerId>,
        expr_id: Option<ExprId>,
        precision: Option<AbsoluteBits>,
        revision_identity: Option<&'static str>,
        public_api_identity: Option<&'static str>,
    ) -> Self {
        Self {
            owner,
            consumer_id,
            expr_id,
            precision,
            revision_identity,
            public_api_identity,
            product_identity: None,
        }
    }

    pub fn with_product_identity(mut self, product_identity: ComputationProductId) -> Self {
        self.product_identity = Some(product_identity);
        self
    }

    pub const fn owner(&self) -> Option<OwnerId> {
        self.owner
    }

    pub const fn consumer_id(&self) -> Option<ConsumerId> {
        self.consumer_id
    }

    pub const fn expr_id(&self) -> Option<ExprId> {
        self.expr_id
    }

    pub const fn precision(&self) -> Option<AbsoluteBits> {
        self.precision
    }

    pub const fn revision_identity(&self) -> Option<&'static str> {
        self.revision_identity
    }

    pub const fn public_api_identity(&self) -> Option<&'static str> {
        self.public_api_identity
    }

    pub fn product_identity(&self) -> Option<ComputationProductId> {
        self.product_identity.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvenanceMismatchField {
    Consumer,
    Owner,
    Expression,
    Precision,
    Source,
    Product,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvenanceMismatchPayload {
    field: ProvenanceMismatchField,
    expected: ProvenanceRecord,
    actual: ProvenanceRecord,
}

impl ProvenanceMismatchPayload {
    pub fn new(
        field: ProvenanceMismatchField,
        expected: ProvenanceRecord,
        actual: ProvenanceRecord,
    ) -> Self {
        Self {
            field,
            expected,
            actual,
        }
    }

    pub const fn field(&self) -> ProvenanceMismatchField {
        self.field
    }

    pub fn expected(&self) -> ProvenanceRecord {
        self.expected.clone()
    }

    pub fn actual(&self) -> ProvenanceRecord {
        self.actual.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatrixInputMismatchField {
    Shape,
    Expression,
    MissingExpression,
    DuplicateExpression,
    ExtraExpression,
    NumericValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatrixInputMismatchPayload {
    field: MatrixInputMismatchField,
    row: Option<usize>,
    column: Option<usize>,
    expected: ProvenanceRecord,
    actual: ProvenanceRecord,
    expected_product: Option<ComputationProductId>,
    actual_product: Option<ComputationProductId>,
    expected_expr: Option<ExprId>,
    actual_expr: Option<ExprId>,
    expected_value_bits: Option<u64>,
    actual_value_bits: Option<u64>,
}

impl MatrixInputMismatchPayload {
    pub fn new(
        field: MatrixInputMismatchField,
        expected: ProvenanceRecord,
        actual: ProvenanceRecord,
    ) -> Self {
        Self {
            field,
            row: None,
            column: None,
            expected,
            actual,
            expected_product: None,
            actual_product: None,
            expected_expr: None,
            actual_expr: None,
            expected_value_bits: None,
            actual_value_bits: None,
        }
    }

    pub const fn field(&self) -> MatrixInputMismatchField {
        self.field
    }

    pub const fn row(&self) -> Option<usize> {
        self.row
    }

    pub const fn column(&self) -> Option<usize> {
        self.column
    }

    pub const fn expected_expr(&self) -> Option<ExprId> {
        self.expected_expr
    }

    pub const fn actual_expr(&self) -> Option<ExprId> {
        self.actual_expr
    }

    pub const fn expected_value_bits(&self) -> Option<u64> {
        self.expected_value_bits
    }

    pub const fn actual_value_bits(&self) -> Option<u64> {
        self.actual_value_bits
    }

    pub fn expected_product_identity(&self) -> Option<ComputationProductId> {
        self.expected_product.clone()
    }

    pub fn actual_product_identity(&self) -> Option<ComputationProductId> {
        self.actual_product.clone()
    }

    pub fn expected(&self) -> ProvenanceRecord {
        self.expected.clone()
    }

    pub fn actual(&self) -> ProvenanceRecord {
        self.actual.clone()
    }

    pub fn with_location(mut self, row: usize, column: usize) -> Self {
        self.row = Some(row);
        self.column = Some(column);
        self
    }

    pub fn with_products(
        mut self,
        expected: Option<ComputationProductId>,
        actual: Option<ComputationProductId>,
    ) -> Self {
        self.expected_product = expected;
        self.actual_product = actual;
        self
    }

    pub const fn with_expressions(
        mut self,
        expected: Option<ExprId>,
        actual: Option<ExprId>,
    ) -> Self {
        self.expected_expr = expected;
        self.actual_expr = actual;
        self
    }

    pub const fn with_value_bits(mut self, expected: Option<u64>, actual: Option<u64>) -> Self {
        self.expected_value_bits = expected;
        self.actual_value_bits = actual;
        self
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum PrecisionFailurePayload {
    EnclosureWidth { actual_width: Dyadic },
    Bigint(BigintError),
    Algnum(AlgnumError),
}

#[derive(Debug, Eq, PartialEq)]
pub enum StorageFailurePayload {
    Storage(StorageError),
    GraphIdExhausted,
    DuplicateId { resource: StorageResource, id: u32 },
    Bigint(BigintError),
    Monomial(MonomialErrorKind),
    FormSum(FormSumErrorKind),
    Algnum(AlgnumError),
}

#[derive(Debug, Eq, PartialEq)]
pub enum UnsupportedFailurePayload {
    RequiredOperation(&'static str),
    SourceIdentity,
    Insert(InsertError),
    Evaluation(EvalError),
    Resolution(ResolveError),
}

#[derive(Debug, Eq, PartialEq)]
pub enum NecoFailure {
    MissingAssignment {
        location: FailureLocation,
    },
    UnknownExpression {
        location: FailureLocation,
    },
    UnknownAtom {
        location: FailureLocation,
    },
    DivisionByZero {
        location: FailureLocation,
    },
    UndefinedPower {
        location: FailureLocation,
    },
    EvenRootOfNegative {
        location: FailureLocation,
    },
    InvalidIsolation {
        location: FailureLocation,
    },
    MultipleTargetRoots {
        location: FailureLocation,
    },
    FloatOutOfRange {
        location: FailureLocation,
    },
    ProvenanceMismatch {
        location: FailureLocation,
        payload: Box<ProvenanceMismatchPayload>,
    },
    MatrixInputMismatch {
        location: FailureLocation,
        payload: Box<MatrixInputMismatchPayload>,
    },
    MissingMatrixInput {
        location: FailureLocation,
    },
    PrecisionExhaustion {
        location: FailureLocation,
        requested: AbsoluteBits,
        payload: Box<PrecisionFailurePayload>,
    },
    StorageFailure {
        location: FailureLocation,
        payload: Box<StorageFailurePayload>,
    },
    UnsupportedRequiredOperation {
        location: FailureLocation,
        payload: Box<UnsupportedFailurePayload>,
    },
}

impl NecoFailure {
    pub fn location(&self) -> &FailureLocation {
        match self {
            Self::MissingAssignment { location }
            | Self::UnknownExpression { location }
            | Self::UnknownAtom { location }
            | Self::DivisionByZero { location }
            | Self::UndefinedPower { location }
            | Self::EvenRootOfNegative { location }
            | Self::InvalidIsolation { location }
            | Self::MultipleTargetRoots { location }
            | Self::FloatOutOfRange { location }
            | Self::ProvenanceMismatch { location, .. }
            | Self::MatrixInputMismatch { location, .. }
            | Self::MissingMatrixInput { location }
            | Self::PrecisionExhaustion { location, .. }
            | Self::StorageFailure { location, .. }
            | Self::UnsupportedRequiredOperation { location, .. } => location,
        }
    }

    pub fn from_graph(location: FailureLocation, error: GraphError) -> Self {
        map_graph_failure(location, error)
    }

    pub fn from_insert(location: FailureLocation, error: InsertError) -> Self {
        map_insert_failure(location, error)
    }

    pub fn from_evaluation(location: FailureLocation, error: EvalError) -> Self {
        map_eval_failure(location, error)
    }

    pub fn from_resolution(location: FailureLocation, error: ResolveError) -> Self {
        map_resolve_failure(location, error)
    }
}

pub(crate) fn map_storage_failure(location: FailureLocation, error: StorageError) -> NecoFailure {
    NecoFailure::StorageFailure {
        location,
        payload: Box::new(StorageFailurePayload::Storage(error)),
    }
}

pub(crate) fn map_graph_failure(location: FailureLocation, error: GraphError) -> NecoFailure {
    match error {
        GraphError::IdExhausted => NecoFailure::StorageFailure {
            location,
            payload: Box::new(StorageFailurePayload::GraphIdExhausted),
        },
        GraphError::InvalidChildId { child, .. } => NecoFailure::UnknownExpression {
            location: location.with_expr(child),
        },
        GraphError::Node(error) => map_eval_failure(location, error),
        GraphError::Storage(error) => map_storage_failure(location, error),
    }
}

pub(crate) fn map_insert_failure(location: FailureLocation, error: InsertError) -> NecoFailure {
    match error {
        InsertError::DuplicateId { resource, id } => NecoFailure::StorageFailure {
            location,
            payload: Box::new(StorageFailurePayload::DuplicateId { resource, id }),
        },
        InsertError::Value(error) => map_eval_failure(location, error),
        InsertError::Storage(error) => map_storage_failure(location, error),
    }
}

pub(crate) fn map_eval_failure(location: FailureLocation, error: EvalError) -> NecoFailure {
    match error {
        EvalError::DivisionByZero => NecoFailure::DivisionByZero { location },
        EvalError::UndefinedZeroPower | EvalError::ZeroToNegativePower => {
            NecoFailure::UndefinedPower { location }
        }
        EvalError::EvenRootOfNegative => NecoFailure::EvenRootOfNegative { location },
        EvalError::Bigint(error) => map_bigint_failure(location, error),
        EvalError::Monomial(error) => map_monomial_failure(location, error),
        EvalError::FormSum(error) => map_formsum_failure(location, error),
        EvalError::Algnum(error) => map_algnum_failure(location, error),
    }
}

pub(crate) fn map_resolve_failure(location: FailureLocation, error: ResolveError) -> NecoFailure {
    match error {
        ResolveError::MissingAssignment { consumer } => NecoFailure::MissingAssignment {
            location: location.with_consumer(consumer),
        },
        ResolveError::UnknownExprId { consumer, expr } => NecoFailure::UnknownExpression {
            location: location.with_consumer(consumer).with_expr(expr),
        },
        ResolveError::UnknownAtomId { expr, atom } => NecoFailure::UnknownAtom {
            location: location.with_expr(expr).with_atom(atom),
        },
        ResolveError::FloatOutOfRange { consumer, expr } => NecoFailure::FloatOutOfRange {
            location: location.with_consumer(consumer).with_expr(expr),
        },
        ResolveError::Evaluation(error) => map_eval_failure(location, error),
        ResolveError::Bigint(error) => map_bigint_failure(location, error),
        ResolveError::Algnum(error) => map_algnum_failure(location, error),
        ResolveError::Storage(error) => map_storage_failure(location, error),
    }
}

fn map_bigint_failure(location: FailureLocation, error: BigintError) -> NecoFailure {
    match error {
        BigintError::DivisionByZero | BigintError::ZeroDenominator => {
            NecoFailure::DivisionByZero { location }
        }
        BigintError::FloatOutOfRange | BigintError::NonFiniteFloat => {
            NecoFailure::FloatOutOfRange { location }
        }
        BigintError::CapacityOverflow
        | BigintError::AllocationFailure { .. }
        | BigintError::ExponentOverflow { .. } => NecoFailure::StorageFailure {
            location,
            payload: Box::new(StorageFailurePayload::Bigint(error)),
        },
        BigintError::UnsignedUnderflow
        | BigintError::NonExactDivision
        | BigintError::InvalidInterval => NecoFailure::UnsupportedRequiredOperation {
            location,
            payload: Box::new(UnsupportedFailurePayload::Evaluation(EvalError::Bigint(
                error,
            ))),
        },
    }
}

fn map_monomial_failure(location: FailureLocation, error: MonomialErrorKind) -> NecoFailure {
    match error {
        MonomialErrorKind::DivisionByZero => NecoFailure::DivisionByZero { location },
        MonomialErrorKind::UndefinedZeroPower | MonomialErrorKind::ZeroToNegativePower => {
            NecoFailure::UndefinedPower { location }
        }
        MonomialErrorKind::EvenRootOfNegative => NecoFailure::EvenRootOfNegative { location },
        MonomialErrorKind::CapacityOverflow | MonomialErrorKind::AllocationFailure { .. } => {
            NecoFailure::StorageFailure {
                location,
                payload: Box::new(StorageFailurePayload::Monomial(error)),
            }
        }
        MonomialErrorKind::Bigint(error) => map_bigint_failure(location, error),
        MonomialErrorKind::InvalidRadicalBasis => NecoFailure::UnsupportedRequiredOperation {
            location,
            payload: Box::new(UnsupportedFailurePayload::Evaluation(EvalError::Monomial(
                error,
            ))),
        },
    }
}

fn map_formsum_failure(location: FailureLocation, error: FormSumErrorKind) -> NecoFailure {
    match error {
        FormSumErrorKind::DivisionByZero => NecoFailure::DivisionByZero { location },
        FormSumErrorKind::DimensionOverflow { .. } | FormSumErrorKind::AllocationFailure { .. } => {
            NecoFailure::StorageFailure {
                location,
                payload: Box::new(StorageFailurePayload::FormSum(error)),
            }
        }
        FormSumErrorKind::Bigint(error) => map_bigint_failure(location, error),
        FormSumErrorKind::Monomial(error) => map_monomial_failure(location, error),
    }
}

fn map_algnum_failure(location: FailureLocation, error: AlgnumError) -> NecoFailure {
    match error {
        AlgnumError::ZeroPolynomial | AlgnumError::InvalidIsolation | AlgnumError::NoTargetRoot => {
            NecoFailure::InvalidIsolation { location }
        }
        AlgnumError::MultipleTargetRoots => NecoFailure::MultipleTargetRoots { location },
        AlgnumError::DivisionByZero => NecoFailure::DivisionByZero { location },
        AlgnumError::UndefinedZeroPower | AlgnumError::ZeroToNegativePower => {
            NecoFailure::UndefinedPower { location }
        }
        AlgnumError::EvenRootOfNegative => NecoFailure::EvenRootOfNegative { location },
        AlgnumError::RepresentationLimit { .. }
        | AlgnumError::AllocationLimit { .. }
        | AlgnumError::AllocationFailure { .. } => NecoFailure::StorageFailure {
            location,
            payload: Box::new(StorageFailurePayload::Algnum(error)),
        },
        AlgnumError::Bigint(error) => map_bigint_failure(location, error),
        AlgnumError::FormSum(error) => map_formsum_failure(location, error),
        AlgnumError::ZeroRootDegree => NecoFailure::UnsupportedRequiredOperation {
            location,
            payload: Box::new(UnsupportedFailurePayload::Evaluation(EvalError::Algnum(
                error,
            ))),
        },
    }
}
