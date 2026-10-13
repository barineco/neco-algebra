use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use neco_algnum::AlgnumError;
use neco_bigint::{BigintError, Sign};

use crate::error::{
    map_eval_failure, map_resolve_failure, map_storage_failure, PrecisionFailurePayload,
    ProvenanceMismatchField, ProvenanceMismatchPayload, ProvenanceRecord,
};
use crate::evaluate::{evaluate_reachable, EvaluationRunError};
use crate::float::{FloatError, PrecisionResolutionError};
use crate::index::{GrowingIndex, SortedIndex};
use crate::storage::{reserve_entries, StorageResource};
use crate::value::{decide_equality, decide_sign, decide_zero};
use crate::{
    AbsoluteBits, AtomStore, CertifiedF64, ConsumerId, EvaluationCache, ExactValue, ExprGraph,
    ExprId, FailureLocation, FailureOperation, IsolationCache, NecoFailure, OwnerId, ResolveError,
    UnsupportedFailurePayload,
};

pub const PUBLIC_API_IDENTITY: &str = "neco-expr-exact-computation-v1";
#[derive(Clone, Debug)]
pub struct ComputationProductId {
    token: Arc<()>,
}

impl ComputationProductId {
    fn new() -> Self {
        Self {
            token: Arc::new(()),
        }
    }
}

impl PartialEq for ComputationProductId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.token, &other.token)
    }
}

impl Eq for ComputationProductId {}

impl core::hash::Hash for ComputationProductId {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.token) as usize).hash(state);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactDecisionKind {
    Zero,
    Equality,
    Sign,
    Degeneracy,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactDecisionValue {
    Zero,
    NonZero,
    Equal,
    NotEqual,
    Negative,
    Positive,
    Degenerate,
    NonDegenerate,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactDecisionRequest {
    Zero(ExprId),
    Equality(ExprId, ExprId),
    Sign(ExprId),
    Degeneracy(ExprId),
}

impl ExactDecisionRequest {
    pub const fn kind(self) -> ExactDecisionKind {
        match self {
            Self::Zero(_) => ExactDecisionKind::Zero,
            Self::Equality(_, _) => ExactDecisionKind::Equality,
            Self::Sign(_) => ExactDecisionKind::Sign,
            Self::Degeneracy(_) => ExactDecisionKind::Degeneracy,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactOperation {
    NormalizeMonomial,
    NormalizeFormSum,
    NormalizeAlgebraic,
    DecideZero,
    DecideEquality,
    DecideSign,
    DecideDegeneracy,
    ResolveCertifiedF64,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ExactExpressionRequirement {
    consumer_id: ConsumerId,
    expressions: Vec<ExprId>,
    decision: ExactDecisionRequest,
    precision: AbsoluteBits,
}

impl ExactExpressionRequirement {
    pub fn new(
        consumer_id: ConsumerId,
        expressions: &[ExprId],
        decision: ExactDecisionRequest,
        precision: AbsoluteBits,
    ) -> Result<Self, NecoFailure> {
        let location =
            FailureLocation::operation(FailureOperation::Allocate).with_consumer(consumer_id);
        if expressions.is_empty() {
            return Err(NecoFailure::MissingAssignment { location });
        }
        let (decision_operands, decision_operand_count) = decision_operands(decision);
        if !decision_operands[..decision_operand_count]
            .iter()
            .all(|operand| expressions.contains(operand))
        {
            return Err(NecoFailure::MissingAssignment { location });
        }
        let mut owned = Vec::new();
        reserve_entries(
            &mut owned,
            expressions.len(),
            StorageResource::ProductEntries,
        )
        .map_err(|error| map_storage_failure(location, error))?;
        owned.extend_from_slice(expressions);
        owned.sort_unstable();
        if owned.windows(2).any(|pair| pair[0] == pair[1]) {
            let positions =
                SortedIndex::build(expressions.iter().copied(), StorageResource::ProductEntries)
                    .map_err(|error| map_storage_failure(location, error))?;
            let duplicate = positions
                .second_occurrences()
                .min()
                .map_or(expressions[0], |position| expressions[position]);
            return Err(NecoFailure::StorageFailure {
                location: location.with_expr(duplicate),
                payload: Box::new(crate::StorageFailurePayload::DuplicateId {
                    resource: StorageResource::ProductEntries,
                    id: duplicate.get(),
                }),
            });
        }
        Ok(Self {
            consumer_id,
            expressions: owned,
            decision,
            precision,
        })
    }

    pub const fn consumer_id(&self) -> ConsumerId {
        self.consumer_id
    }

    pub fn expressions(&self) -> &[ExprId] {
        &self.expressions
    }

    pub const fn decision(&self) -> ExactDecisionRequest {
        self.decision
    }

    pub const fn precision(&self) -> AbsoluteBits {
        self.precision
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct NecoObservedCapability {
    source_identity: &'static str,
    supported_operations: Vec<ExactOperation>,
}

impl NecoObservedCapability {
    pub fn current(supported_operations: &[ExactOperation]) -> Result<Self, NecoFailure> {
        Self::new(env!("CARGO_PKG_VERSION"), supported_operations)
    }

    pub fn new(
        source_identity: &'static str,
        supported_operations: &[ExactOperation],
    ) -> Result<Self, NecoFailure> {
        let location = FailureLocation::operation(FailureOperation::Read);
        let mut supported = Vec::new();
        reserve_entries(
            &mut supported,
            supported_operations.len(),
            StorageResource::ProductEntries,
        )
        .map_err(|error| map_storage_failure(location, error))?;
        supported.extend_from_slice(supported_operations);
        supported.sort_unstable();
        supported.dedup();
        Ok(Self {
            source_identity,
            supported_operations: supported,
        })
    }

    pub const fn source_identity(&self) -> &'static str {
        self.source_identity
    }

    pub fn supported_operations(&self) -> &[ExactOperation] {
        &self.supported_operations
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NecoImplementationSource {
    revision_identity: &'static str,
    public_api_identity: &'static str,
}

impl NecoImplementationSource {
    pub const fn new(revision_identity: &'static str, public_api_identity: &'static str) -> Self {
        Self {
            revision_identity,
            public_api_identity,
        }
    }

    pub const fn current() -> Self {
        Self::new(env!("CARGO_PKG_VERSION"), PUBLIC_API_IDENTITY)
    }

    pub const fn revision_identity(&self) -> &'static str {
        self.revision_identity
    }

    pub const fn public_api_identity(&self) -> &'static str {
        self.public_api_identity
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct OwnerExpressionSet {
    owner: OwnerId,
    graph: ExprGraph,
    atoms: AtomStore,
    requirements: Vec<ExactExpressionRequirement>,
}

impl OwnerExpressionSet {
    pub fn new(
        owner: OwnerId,
        graph: ExprGraph,
        atoms: AtomStore,
        requirements: Vec<ExactExpressionRequirement>,
    ) -> Result<Self, NecoFailure> {
        for requirement in &requirements {
            for expression in requirement.expressions() {
                if graph.get(*expression).is_none() {
                    return Err(NecoFailure::UnknownExpression {
                        location: FailureLocation::operation(FailureOperation::Read)
                            .with_consumer(requirement.consumer_id())
                            .with_expr(*expression),
                    });
                }
            }
        }
        Ok(Self {
            owner,
            graph,
            atoms,
            requirements,
        })
    }

    pub const fn owner(&self) -> OwnerId {
        self.owner
    }
    pub fn graph(&self) -> &ExprGraph {
        &self.graph
    }
    pub fn atoms(&self) -> &AtomStore {
        &self.atoms
    }
    pub fn requirements(&self) -> &[ExactExpressionRequirement] {
        &self.requirements
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ExactAllocationInput {
    owners: Vec<OwnerExpressionSet>,
    observed_capability: NecoObservedCapability,
    implementation_source: NecoImplementationSource,
}

impl ExactAllocationInput {
    pub const fn new(
        owners: Vec<OwnerExpressionSet>,
        observed_capability: NecoObservedCapability,
        implementation_source: NecoImplementationSource,
    ) -> Self {
        Self {
            owners,
            observed_capability,
            implementation_source,
        }
    }

    pub fn owners(&self) -> &[OwnerExpressionSet] {
        &self.owners
    }
    pub fn observed_capability(&self) -> &NecoObservedCapability {
        &self.observed_capability
    }
    pub fn implementation_source(&self) -> &NecoImplementationSource {
        &self.implementation_source
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ExactNumericAllocation {
    descent_consumers: Vec<(OwnerId, ConsumerId)>,
}

impl ExactNumericAllocation {
    pub fn descent_consumers(&self) -> &[(OwnerId, ConsumerId)] {
        &self.descent_consumers
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct AllocatedExactNumericContext {
    input: ExactAllocationInput,
    allocation: ExactNumericAllocation,
}

impl AllocatedExactNumericContext {
    pub fn allocation(&self) -> &ExactNumericAllocation {
        &self.allocation
    }
}

#[derive(Debug, Eq, PartialEq)]
struct NormalFormEntry {
    owner: OwnerId,
    consumer: ConsumerId,
    expr: ExprId,
    value_position: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub struct NormalizedExactNumericContext {
    allocated: AllocatedExactNumericContext,
    normal_values: Vec<Arc<ExactValue>>,
    normal_forms: Vec<NormalFormEntry>,
    normal_form_index: SortedIndex<(OwnerId, ConsumerId, ExprId)>,
    evaluation_count: usize,
}

impl NormalizedExactNumericContext {
    pub fn normal_form(
        &self,
        owner: OwnerId,
        consumer: ConsumerId,
        expr: ExprId,
    ) -> Option<&ExactValue> {
        self.normal_form_shared(owner, consumer, expr)
            .map(|value| &**value)
    }

    fn normal_form_shared(
        &self,
        owner: OwnerId,
        consumer: ConsumerId,
        expr: ExprId,
    ) -> Option<&Arc<ExactValue>> {
        self.normal_form_index
            .first(&(owner, consumer, expr))
            .map(|position| &self.normal_values[self.normal_forms[position].value_position])
    }

    pub const fn evaluation_count(&self) -> usize {
        self.evaluation_count
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ExactDecisionWitness {
    kind: ExactDecisionKind,
    operands: Vec<ExprId>,
    value: ExactDecisionValue,
    normal_form_witness: Arc<ExactValue>,
}

impl ExactDecisionWitness {
    pub const fn kind(&self) -> ExactDecisionKind {
        self.kind
    }

    pub fn operands(&self) -> &[ExprId] {
        &self.operands
    }

    pub const fn value(&self) -> ExactDecisionValue {
        self.value
    }

    pub fn normal_form_witness(&self) -> &ExactValue {
        &self.normal_form_witness
    }
}

#[derive(Debug, Eq, PartialEq)]
struct DecisionEntry {
    owner: OwnerId,
    consumer: ConsumerId,
    witness: ExactDecisionWitness,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DecidedExactNumericContext {
    normalized: NormalizedExactNumericContext,
    decisions: Vec<DecisionEntry>,
    decision_index: SortedIndex<(OwnerId, ConsumerId)>,
}

impl DecidedExactNumericContext {
    pub fn decision(&self, owner: OwnerId, consumer: ConsumerId) -> Option<&ExactDecisionWitness> {
        self.decision_index
            .first(&(owner, consumer))
            .map(|position| &self.decisions[position].witness)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct CertifiedF64Expression {
    owner: OwnerId,
    consumer_id: ConsumerId,
    expr: ExprId,
    precision: AbsoluteBits,
    source: NecoImplementationSource,
    certified: Arc<CertifiedF64>,
}

impl CertifiedF64Expression {
    pub const fn owner(&self) -> OwnerId {
        self.owner
    }

    pub const fn consumer_id(&self) -> ConsumerId {
        self.consumer_id
    }

    pub const fn expr(&self) -> ExprId {
        self.expr
    }

    pub const fn precision(&self) -> AbsoluteBits {
        self.precision
    }

    pub const fn source(&self) -> NecoImplementationSource {
        self.source
    }

    pub fn certified(&self) -> &CertifiedF64 {
        &self.certified
    }
    fn try_clone(&self) -> Result<Self, NecoFailure> {
        Ok(Self {
            owner: self.owner,
            consumer_id: self.consumer_id,
            expr: self.expr,
            precision: self.precision,
            source: self.source,
            certified: Arc::clone(&self.certified),
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct CertifiedF64Bundle {
    owner: OwnerId,
    consumer_id: ConsumerId,
    precision: AbsoluteBits,
    source: NecoImplementationSource,
    product_identity: ComputationProductId,
    values: Vec<CertifiedF64Expression>,
    value_index: SortedIndex<ExprId>,
}

#[derive(Default)]
struct ResolutionCache {
    entries: Vec<Arc<CertifiedF64>>,
    index: GrowingIndex<(ExprId, AbsoluteBits)>,
}

fn bundle_value_index(
    values: &[CertifiedF64Expression],
    location: FailureLocation,
) -> Result<SortedIndex<ExprId>, NecoFailure> {
    SortedIndex::build(
        values.iter().map(|value| value.expr),
        StorageResource::ProductResolutionEntries,
    )
    .map_err(|error| map_storage_failure(location, error))
}

impl CertifiedF64Bundle {
    pub const fn owner(&self) -> OwnerId {
        self.owner
    }

    pub const fn consumer_id(&self) -> ConsumerId {
        self.consumer_id
    }

    pub const fn precision(&self) -> AbsoluteBits {
        self.precision
    }

    pub const fn source(&self) -> NecoImplementationSource {
        self.source
    }

    pub fn values(&self) -> &[CertifiedF64Expression] {
        &self.values
    }
    pub fn product_identity(&self) -> &ComputationProductId {
        &self.product_identity
    }

    pub fn try_clone(&self) -> Result<Self, NecoFailure> {
        let location =
            FailureLocation::operation(FailureOperation::Resolve).with_consumer(self.consumer_id);
        let mut values = Vec::new();
        reserve_entries(
            &mut values,
            self.values.len(),
            StorageResource::ProductResolutionEntries,
        )
        .map_err(|error| map_storage_failure(location, error))?;
        for value in &self.values {
            values.push(value.try_clone()?);
        }
        let value_index = bundle_value_index(&values, location)?;
        Ok(Self {
            owner: self.owner,
            consumer_id: self.consumer_id,
            precision: self.precision,
            source: self.source,
            product_identity: self.product_identity.clone(),
            values,
            value_index,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ResolvedExactNumericContext {
    decided: DecidedExactNumericContext,
    certified_descents: Vec<CertifiedF64Bundle>,
    descent_index: SortedIndex<(OwnerId, ConsumerId)>,
    shared_resolution_count: usize,
    product_identity: ComputationProductId,
}

impl ResolvedExactNumericContext {
    pub fn certified_descent(
        &self,
        owner: OwnerId,
        consumer: ConsumerId,
    ) -> Option<&CertifiedF64Bundle> {
        self.descent_index
            .first(&(owner, consumer))
            .map(|position| &self.certified_descents[position])
    }

    pub const fn shared_resolution_count(&self) -> usize {
        self.shared_resolution_count
    }
    pub fn product_identity(&self) -> &ComputationProductId {
        &self.product_identity
    }
}

pub struct DirectInspection<'a> {
    allocation: &'a ExactNumericAllocation,
    requirements: Vec<(OwnerId, &'a ExactExpressionRequirement)>,
    decisions: &'a [DecisionEntry],
    decision_index: &'a SortedIndex<(OwnerId, ConsumerId)>,
    certified_descents: &'a [CertifiedF64Bundle],
    shared_resolution_count: usize,
}

impl DirectInspection<'_> {
    pub fn allocation(&self) -> &ExactNumericAllocation {
        self.allocation
    }

    pub fn requirements(&self) -> &[(OwnerId, &ExactExpressionRequirement)] {
        &self.requirements
    }

    pub fn certified_descents(&self) -> &[CertifiedF64Bundle] {
        self.certified_descents
    }

    pub const fn shared_resolution_count(&self) -> usize {
        self.shared_resolution_count
    }

    pub fn decision(&self, owner: OwnerId, consumer: ConsumerId) -> Option<&ExactDecisionWitness> {
        self.decision_index
            .first(&(owner, consumer))
            .map(|position| &self.decisions[position].witness)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ExactComputationProduct {
    resolved: ResolvedExactNumericContext,
}

impl ExactComputationProduct {
    pub fn allocation(&self) -> &ExactNumericAllocation {
        &self.resolved.decided.normalized.allocated.allocation
    }

    pub fn normal_form(
        &self,
        owner: OwnerId,
        consumer: ConsumerId,
        expr: ExprId,
    ) -> Option<&ExactValue> {
        self.resolved
            .decided
            .normalized
            .normal_form(owner, consumer, expr)
    }

    pub fn decision(&self, owner: OwnerId, consumer: ConsumerId) -> Option<&ExactDecisionWitness> {
        self.resolved.decided.decision(owner, consumer)
    }

    pub fn certified_descent(
        &self,
        owner: OwnerId,
        consumer: ConsumerId,
    ) -> Option<&CertifiedF64Bundle> {
        self.resolved.certified_descent(owner, consumer)
    }
    pub fn product_identity(&self) -> &ComputationProductId {
        self.resolved.product_identity()
    }

    pub fn direct_inspection(&self) -> Result<DirectInspection<'_>, NecoFailure> {
        let owners = &self.resolved.decided.normalized.allocated.input.owners;
        let total = requirement_count(owners, FailureOperation::Assemble)?;
        let mut requirements = Vec::new();
        reserve_entries(&mut requirements, total, StorageResource::ProductEntries).map_err(
            |error| {
                map_storage_failure(
                    FailureLocation::operation(FailureOperation::Assemble),
                    error,
                )
            },
        )?;
        for core in owners {
            requirements.extend(
                core.requirements
                    .iter()
                    .map(|requirement| (core.owner, requirement)),
            );
        }
        Ok(DirectInspection {
            allocation: self.allocation(),
            requirements,
            decisions: &self.resolved.decided.decisions,
            decision_index: &self.resolved.decided.decision_index,
            certified_descents: &self.resolved.certified_descents,
            shared_resolution_count: self.resolved.shared_resolution_count,
        })
    }

    pub const fn as_exact_computation(&self) -> &Self {
        self
    }
}

pub fn read_exact_numeric_inputs(
    input: ExactAllocationInput,
) -> Result<ExactAllocationInput, NecoFailure> {
    validate_owners(&input.owners, FailureOperation::Read)?;
    let expected_source = NecoImplementationSource::current();
    if input.implementation_source != expected_source {
        return Err(source_provenance_mismatch(
            FailureLocation::operation(FailureOperation::Read),
            expected_source,
            input.implementation_source,
        ));
    }
    if input.observed_capability.source_identity != env!("CARGO_PKG_VERSION") {
        return Err(source_provenance_mismatch(
            FailureLocation::operation(FailureOperation::Read),
            expected_source,
            NecoImplementationSource::new(
                input.observed_capability.source_identity,
                input.implementation_source.public_api_identity,
            ),
        ));
    }
    Ok(input)
}

pub fn allocate_exact_numeric(
    input: ExactAllocationInput,
) -> Result<AllocatedExactNumericContext, NecoFailure> {
    let required_operations = [
        ExactOperation::NormalizeMonomial,
        ExactOperation::NormalizeFormSum,
        ExactOperation::NormalizeAlgebraic,
        ExactOperation::DecideZero,
        ExactOperation::DecideEquality,
        ExactOperation::DecideSign,
        ExactOperation::DecideDegeneracy,
        ExactOperation::ResolveCertifiedF64,
    ];
    for operation in required_operations {
        if input
            .observed_capability
            .supported_operations
            .binary_search(&operation)
            .is_err()
        {
            return Err(NecoFailure::UnsupportedRequiredOperation {
                location: FailureLocation::operation(FailureOperation::Allocate),
                payload: Box::new(UnsupportedFailurePayload::RequiredOperation(
                    operation_name(operation),
                )),
            });
        }
    }
    let mut consumers = Vec::new();
    let mut seen_consumers = GrowingIndex::default();
    validate_owners(&input.owners, FailureOperation::Allocate)?;
    {
        let mut collect_consumers =
            |owner: OwnerId, requirements: &[ExactExpressionRequirement]| {
                for requirement in requirements {
                    let key = (owner, requirement.consumer_id);
                    if seen_consumers.get(&key).is_some() {
                        return Err(NecoFailure::StorageFailure {
                            location: FailureLocation::operation(FailureOperation::Allocate)
                                .with_consumer(requirement.consumer_id),
                            payload: Box::new(crate::StorageFailurePayload::DuplicateId {
                                resource: StorageResource::ProductEntries,
                                id: requirement.consumer_id.get(),
                            }),
                        });
                    }
                    let required = consumers.len().checked_add(1).ok_or_else(|| {
                        map_storage_failure(
                            FailureLocation::operation(FailureOperation::Allocate),
                            crate::StorageError::CapacityOverflow {
                                resource: StorageResource::ProductEntries,
                            },
                        )
                    })?;
                    reserve_entries(&mut consumers, required, StorageResource::ProductEntries)
                        .map_err(|error| {
                            map_storage_failure(
                                FailureLocation::operation(FailureOperation::Allocate),
                                error,
                            )
                        })?;
                    seen_consumers
                        .insert(key, consumers.len(), StorageResource::ProductEntries)
                        .map_err(|error| {
                            map_storage_failure(
                                FailureLocation::operation(FailureOperation::Allocate),
                                error,
                            )
                        })?;
                    consumers.push(key);
                }
                Ok::<(), NecoFailure>(())
            };
        for core in &input.owners {
            collect_consumers(core.owner, &core.requirements)?;
        }
    }
    consumers.sort_unstable();
    let allocation = ExactNumericAllocation {
        descent_consumers: consumers,
    };
    Ok(AllocatedExactNumericContext { input, allocation })
}

pub fn normalize_exact_expressions(
    allocated: AllocatedExactNumericContext,
) -> Result<NormalizedExactNumericContext, NecoFailure> {
    #[cfg(test)]
    if let Some(error) = injected_operation_failure(FailureOperation::Normalize) {
        return Err(error);
    }
    let mut normal_values = Vec::new();
    let mut normal_forms = Vec::new();
    let mut evaluation_count = 0_usize;
    for core in &allocated.input.owners {
        normalize_core(
            core.owner,
            core,
            &mut normal_values,
            &mut normal_forms,
            &mut evaluation_count,
        )?;
    }
    let normal_form_index = SortedIndex::build(
        normal_forms
            .iter()
            .map(|entry| (entry.owner, entry.consumer, entry.expr)),
        StorageResource::ProductEntries,
    )
    .map_err(|error| {
        map_storage_failure(
            FailureLocation::operation(FailureOperation::Normalize),
            error,
        )
    })?;
    Ok(NormalizedExactNumericContext {
        allocated,
        normal_values,
        normal_forms,
        normal_form_index,
        evaluation_count,
    })
}

pub fn decide_exact_properties(
    normalized: NormalizedExactNumericContext,
) -> Result<DecidedExactNumericContext, NecoFailure> {
    #[cfg(test)]
    if let Some(error) = injected_operation_failure(FailureOperation::Decide) {
        return Err(error);
    }
    let mut decisions = Vec::new();
    for core in &normalized.allocated.input.owners {
        decide_core(core.owner, &normalized, &core.requirements, &mut decisions)?;
    }
    let decision_index = SortedIndex::build(
        decisions.iter().map(|entry| (entry.owner, entry.consumer)),
        StorageResource::ProductEntries,
    )
    .map_err(|error| {
        map_storage_failure(FailureLocation::operation(FailureOperation::Decide), error)
    })?;
    Ok(DecidedExactNumericContext {
        normalized,
        decisions,
        decision_index,
    })
}

fn decide_core(
    owner: OwnerId,
    normalized: &NormalizedExactNumericContext,
    requirements: &[ExactExpressionRequirement],
    decisions: &mut Vec<DecisionEntry>,
) -> Result<(), NecoFailure> {
    for requirement in requirements {
        let location = FailureLocation::operation(FailureOperation::Decide)
            .with_consumer(requirement.consumer_id)
            .with_decision(requirement.consumer_id.get());
        let (operands, operand_count) = decision_operands(requirement.decision);
        let operands = &operands[..operand_count];
        let first_shared = normalized
            .normal_form_shared(owner, requirement.consumer_id, operands[0])
            .ok_or(NecoFailure::UnknownExpression {
                location: location.with_expr(operands[0]),
            })?;
        let first = &**first_shared;
        let (value, witness) = match requirement.decision {
            ExactDecisionRequest::Zero(_) => (
                if decide_zero(first) {
                    ExactDecisionValue::Zero
                } else {
                    ExactDecisionValue::NonZero
                },
                Arc::clone(first_shared),
            ),
            ExactDecisionRequest::Equality(_, right) => {
                let right_value = normalized
                    .normal_form(owner, requirement.consumer_id, right)
                    .ok_or(NecoFailure::UnknownExpression {
                        location: location.with_expr(right),
                    })?;
                let equal = decide_equality(first, right_value)
                    .map_err(|error| map_eval_failure(location, error))?;
                let difference = crate::value::sub_exact(first, right_value)
                    .map_err(|error| map_eval_failure(location, error))?;
                (
                    if equal {
                        ExactDecisionValue::Equal
                    } else {
                        ExactDecisionValue::NotEqual
                    },
                    Arc::new(difference),
                )
            }
            ExactDecisionRequest::Sign(_) => (
                match decide_sign(first).map_err(|error| map_eval_failure(location, error))? {
                    Sign::Negative => ExactDecisionValue::Negative,
                    Sign::Zero => ExactDecisionValue::Zero,
                    Sign::Positive => ExactDecisionValue::Positive,
                },
                Arc::clone(first_shared),
            ),
            ExactDecisionRequest::Degeneracy(_) => (
                if decide_zero(first) {
                    ExactDecisionValue::Degenerate
                } else {
                    ExactDecisionValue::NonDegenerate
                },
                Arc::clone(first_shared),
            ),
        };
        let mut operand_values = Vec::new();
        reserve_entries(
            &mut operand_values,
            operands.len(),
            StorageResource::ProductEntries,
        )
        .map_err(|error| map_storage_failure(location, error))?;
        operand_values.extend_from_slice(operands);
        let required = decisions.len().checked_add(1).ok_or_else(|| {
            map_storage_failure(
                location,
                crate::StorageError::CapacityOverflow {
                    resource: StorageResource::ProductEntries,
                },
            )
        })?;
        reserve_entries(decisions, required, StorageResource::ProductEntries)
            .map_err(|error| map_storage_failure(location, error))?;
        decisions.push(DecisionEntry {
            owner,
            consumer: requirement.consumer_id,
            witness: ExactDecisionWitness {
                kind: requirement.decision.kind(),
                operands: operand_values,
                value,
                normal_form_witness: witness,
            },
        });
    }
    Ok(())
}

pub fn resolve_certified_f64(
    decided: DecidedExactNumericContext,
) -> Result<ResolvedExactNumericContext, NecoFailure> {
    #[cfg(test)]
    if let Some(error) = injected_operation_failure(FailureOperation::Resolve) {
        return Err(error);
    }
    let mut shared_resolution_count = 0_usize;
    let mut bundles = Vec::new();
    let product_identity = ComputationProductId::new();
    for core in &decided.normalized.allocated.input.owners {
        resolve_core(
            core.owner,
            &decided,
            &core.requirements,
            &mut shared_resolution_count,
            &product_identity,
            &mut bundles,
        )?;
    }
    let descent_index = SortedIndex::build(
        bundles
            .iter()
            .map(|bundle| (bundle.owner, bundle.consumer_id)),
        StorageResource::ProductEntries,
    )
    .map_err(|error| {
        map_storage_failure(FailureLocation::operation(FailureOperation::Resolve), error)
    })?;
    Ok(ResolvedExactNumericContext {
        decided,
        certified_descents: bundles,
        descent_index,
        shared_resolution_count,
        product_identity,
    })
}

fn resolve_core(
    owner: OwnerId,
    decided: &DecidedExactNumericContext,
    requirements: &[ExactExpressionRequirement],
    shared_resolution_count: &mut usize,
    product_identity: &ComputationProductId,
    bundles: &mut Vec<CertifiedF64Bundle>,
) -> Result<(), NecoFailure> {
    let source = decided.normalized.allocated.input.implementation_source;
    let mut cache = ResolutionCache::default();
    for requirement in requirements {
        let location = FailureLocation::operation(FailureOperation::Resolve)
            .with_consumer(requirement.consumer_id);
        let mut values = Vec::new();
        reserve_entries(
            &mut values,
            requirement.expressions.len(),
            StorageResource::ProductResolutionEntries,
        )
        .map_err(|error| map_storage_failure(location, error))?;
        for expression in &requirement.expressions {
            let cache_key = (*expression, requirement.precision);
            let certified = match cache
                .index
                .get(&cache_key)
                .map(|position| &cache.entries[position])
            {
                Some(entry) => Arc::clone(entry),
                None => {
                    let exact = decided
                        .normalized
                        .normal_form(owner, requirement.consumer_id, *expression)
                        .ok_or(NecoFailure::UnknownExpression {
                            location: location.with_expr(*expression),
                        })?;
                    let resolved = Arc::new(
                        CertifiedF64::resolve_for_requirement(
                            exact,
                            *expression,
                            requirement.precision,
                            &mut IsolationCache::new(),
                        )
                        .map_err(|error| {
                            map_float_failure(location.with_expr(*expression), error)
                        })?,
                    );
                    let required = cache.entries.len().checked_add(1).ok_or_else(|| {
                        map_storage_failure(
                            location,
                            crate::StorageError::CapacityOverflow {
                                resource: StorageResource::ProductResolutionEntries,
                            },
                        )
                    })?;
                    reserve_entries(
                        &mut cache.entries,
                        required,
                        StorageResource::ProductResolutionEntries,
                    )
                    .map_err(|error| map_storage_failure(location, error))?;
                    cache
                        .index
                        .insert(
                            cache_key,
                            cache.entries.len(),
                            StorageResource::ProductResolutionEntries,
                        )
                        .map_err(|error| map_storage_failure(location, error))?;
                    cache.entries.push(Arc::clone(&resolved));
                    resolved
                }
            };
            values.push(CertifiedF64Expression {
                owner,
                consumer_id: requirement.consumer_id,
                expr: *expression,
                precision: requirement.precision,
                source,
                certified,
            });
        }
        let required = bundles.len().checked_add(1).ok_or_else(|| {
            map_storage_failure(
                location,
                crate::StorageError::CapacityOverflow {
                    resource: StorageResource::ProductEntries,
                },
            )
        })?;
        reserve_entries(bundles, required, StorageResource::ProductEntries)
            .map_err(|error| map_storage_failure(location, error))?;
        let value_index = bundle_value_index(&values, location)?;
        bundles.push(CertifiedF64Bundle {
            owner,
            consumer_id: requirement.consumer_id,
            precision: requirement.precision,
            source,
            values,
            value_index,
            product_identity: product_identity.clone(),
        });
    }
    *shared_resolution_count += cache.entries.len();
    Ok(())
}

pub fn assemble_exact_computation_product(
    resolved: ResolvedExactNumericContext,
) -> Result<ExactComputationProduct, NecoFailure> {
    let owners = &resolved.decided.normalized.allocated.input.owners;
    let expected = requirement_count(owners, FailureOperation::Assemble)?;
    if resolved.certified_descents.len() != expected || resolved.decided.decisions.len() != expected
    {
        return Err(NecoFailure::MissingAssignment {
            location: FailureLocation::operation(FailureOperation::Assemble),
        });
    }
    let source = resolved
        .decided
        .normalized
        .allocated
        .input
        .implementation_source;
    for core in owners {
        validate_decision_group(
            core.owner,
            &core.requirements,
            &resolved.decided.decision_index,
        )?;
        validate_result_group(
            core.owner,
            &core.requirements,
            source,
            &resolved.certified_descents,
            &resolved.descent_index,
        )?;
    }
    Ok(ExactComputationProduct { resolved })
}

fn validate_decision_group(
    owner: OwnerId,
    requirements: &[ExactExpressionRequirement],
    decision_index: &SortedIndex<(OwnerId, ConsumerId)>,
) -> Result<(), NecoFailure> {
    for requirement in requirements {
        let location = FailureLocation::operation(FailureOperation::Assemble)
            .with_consumer(requirement.consumer_id);
        if decision_index
            .first(&(owner, requirement.consumer_id))
            .is_none()
        {
            return Err(NecoFailure::MissingAssignment { location });
        }
    }
    Ok(())
}

fn validate_result_group(
    owner: OwnerId,
    requirements: &[ExactExpressionRequirement],
    source: NecoImplementationSource,
    bundles: &[CertifiedF64Bundle],
    descent_index: &SortedIndex<(OwnerId, ConsumerId)>,
) -> Result<(), NecoFailure> {
    for requirement in requirements {
        let key = (owner, requirement.consumer_id);
        let location = FailureLocation::operation(FailureOperation::Assemble)
            .with_consumer(requirement.consumer_id);
        let Some(position) = descent_index.first(&key) else {
            if let Some(actual) = bundles
                .iter()
                .find(|bundle| bundle.consumer_id == requirement.consumer_id)
                .or_else(|| bundles.iter().find(|bundle| bundle.owner == owner))
                .or_else(|| bundles.first())
            {
                return compare_provenance(
                    location,
                    provenance_record(owner, requirement, None, source),
                    bundle_provenance(actual),
                );
            }
            return Err(NecoFailure::MissingAssignment { location });
        };
        let bundle = &bundles[position];
        if descent_index.count(&key) > 1 {
            return Err(provenance_mismatch(
                location,
                provenance_record(owner, requirement, None, source),
                bundle_provenance(bundle),
                ProvenanceMismatchField::Consumer,
            ));
        }
        validate_bundle(owner, requirement, source, bundle)?;
    }
    Ok(())
}

fn validate_bundle(
    owner: OwnerId,
    requirement: &ExactExpressionRequirement,
    source: NecoImplementationSource,
    bundle: &CertifiedF64Bundle,
) -> Result<(), NecoFailure> {
    let bundle_location = FailureLocation::operation(FailureOperation::Assemble)
        .with_consumer(requirement.consumer_id);
    let expected_bundle = provenance_record(owner, requirement, None, source);
    let actual_bundle = bundle_provenance(bundle);
    compare_provenance(bundle_location, expected_bundle, actual_bundle)?;
    for expression in &requirement.expressions {
        let location = bundle_location.with_expr(*expression);
        let Some(position) = bundle.value_index.first(expression) else {
            if let Some(actual) = bundle.values.iter().find(|value| value.expr != *expression) {
                return Err(provenance_mismatch(
                    location,
                    provenance_record(owner, requirement, Some(*expression), source),
                    expression_provenance(actual),
                    ProvenanceMismatchField::Expression,
                ));
            }
            return Err(NecoFailure::MissingAssignment { location });
        };
        let value = &bundle.values[position];
        if bundle.value_index.count(expression) > 1 {
            return Err(provenance_mismatch(
                location,
                provenance_record(owner, requirement, Some(*expression), source),
                expression_provenance(value),
                ProvenanceMismatchField::Expression,
            ));
        }
        compare_provenance(
            location,
            provenance_record(owner, requirement, Some(*expression), source),
            expression_provenance(value),
        )?;
    }
    for value in &bundle.values {
        if requirement.expressions.binary_search(&value.expr).is_err() {
            return Err(provenance_mismatch(
                bundle_location.with_expr(value.expr),
                provenance_record(owner, requirement, None, source),
                expression_provenance(value),
                ProvenanceMismatchField::Expression,
            ));
        }
    }
    Ok(())
}

fn compare_provenance(
    location: FailureLocation,
    expected: ProvenanceRecord,
    actual: ProvenanceRecord,
) -> Result<(), NecoFailure> {
    let field = if expected.consumer_id() != actual.consumer_id() {
        ProvenanceMismatchField::Consumer
    } else if expected.owner() != actual.owner() {
        ProvenanceMismatchField::Owner
    } else if expected.expr_id() != actual.expr_id() {
        ProvenanceMismatchField::Expression
    } else if expected.precision() != actual.precision() {
        ProvenanceMismatchField::Precision
    } else if expected.revision_identity() != actual.revision_identity()
        || expected.public_api_identity() != actual.public_api_identity()
    {
        ProvenanceMismatchField::Source
    } else {
        return Ok(());
    };
    Err(provenance_mismatch(location, expected, actual, field))
}

fn provenance_mismatch(
    location: FailureLocation,
    expected: ProvenanceRecord,
    actual: ProvenanceRecord,
    field: ProvenanceMismatchField,
) -> NecoFailure {
    NecoFailure::ProvenanceMismatch {
        location,
        payload: Box::new(ProvenanceMismatchPayload::new(field, expected, actual)),
    }
}

fn provenance_record(
    owner: OwnerId,
    requirement: &ExactExpressionRequirement,
    expr: Option<ExprId>,
    source: NecoImplementationSource,
) -> ProvenanceRecord {
    ProvenanceRecord::new(
        Some(owner),
        Some(requirement.consumer_id),
        expr,
        Some(requirement.precision),
        Some(source.revision_identity),
        Some(source.public_api_identity),
    )
}

fn bundle_provenance(bundle: &CertifiedF64Bundle) -> ProvenanceRecord {
    ProvenanceRecord::new(
        Some(bundle.owner),
        Some(bundle.consumer_id),
        None,
        Some(bundle.precision),
        Some(bundle.source.revision_identity),
        Some(bundle.source.public_api_identity),
    )
}

fn expression_provenance(value: &CertifiedF64Expression) -> ProvenanceRecord {
    ProvenanceRecord::new(
        Some(value.owner),
        Some(value.consumer_id),
        Some(value.expr),
        Some(value.precision),
        Some(value.source.revision_identity),
        Some(value.source.public_api_identity),
    )
}

fn source_provenance_mismatch(
    location: FailureLocation,
    expected: NecoImplementationSource,
    actual: NecoImplementationSource,
) -> NecoFailure {
    provenance_mismatch(
        location,
        ProvenanceRecord::new(
            None,
            None,
            None,
            None,
            Some(expected.revision_identity),
            Some(expected.public_api_identity),
        ),
        ProvenanceRecord::new(
            None,
            None,
            None,
            None,
            Some(actual.revision_identity),
            Some(actual.public_api_identity),
        ),
        ProvenanceMismatchField::Source,
    )
}

fn normalize_core(
    owner: OwnerId,
    core: &OwnerExpressionSet,
    values: &mut Vec<Arc<ExactValue>>,
    output: &mut Vec<NormalFormEntry>,
    evaluation_count: &mut usize,
) -> Result<(), NecoFailure> {
    let mut cache = EvaluationCache::new();
    let mut positions = GrowingIndex::<ExprId>::default();
    for requirement in core.requirements() {
        for expression in requirement.expressions() {
            let location = FailureLocation::operation(FailureOperation::Normalize)
                .with_consumer(requirement.consumer_id())
                .with_expr(*expression);
            let value_position = match positions.get(expression) {
                Some(position) => position,
                None => {
                    let before = cache.len();
                    let value =
                        evaluate_reachable(*expression, core.graph(), core.atoms(), &mut cache)
                            .map_err(|error| map_evaluation_run_failure(location, error))?;
                    *evaluation_count = evaluation_count
                        .checked_add(cache.len() - before)
                        .ok_or_else(|| {
                            map_storage_failure(
                                location,
                                crate::StorageError::CapacityOverflow {
                                    resource: StorageResource::ProductEntries,
                                },
                            )
                        })?;
                    let required = values.len().checked_add(1).ok_or_else(|| {
                        map_storage_failure(
                            location,
                            crate::StorageError::CapacityOverflow {
                                resource: StorageResource::ProductEntries,
                            },
                        )
                    })?;
                    reserve_entries(values, required, StorageResource::ProductEntries)
                        .map_err(|error| map_storage_failure(location, error))?;
                    positions
                        .insert(*expression, values.len(), StorageResource::ProductEntries)
                        .map_err(|error| map_storage_failure(location, error))?;
                    values.push(Arc::new(value));
                    values.len() - 1
                }
            };
            let required = output.len().checked_add(1).ok_or_else(|| {
                map_storage_failure(
                    location,
                    crate::StorageError::CapacityOverflow {
                        resource: StorageResource::ProductEntries,
                    },
                )
            })?;
            reserve_entries(output, required, StorageResource::ProductEntries)
                .map_err(|error| map_storage_failure(location, error))?;
            output.push(NormalFormEntry {
                owner,
                consumer: requirement.consumer_id(),
                expr: *expression,
                value_position,
            });
        }
    }
    Ok(())
}

fn validate_owners(
    owners: &[OwnerExpressionSet],
    operation: FailureOperation,
) -> Result<(), NecoFailure> {
    for (position, core) in owners.iter().enumerate() {
        if owners[..position]
            .iter()
            .any(|previous| previous.owner == core.owner)
        {
            return Err(NecoFailure::StorageFailure {
                location: FailureLocation::operation(operation),
                payload: Box::new(crate::StorageFailurePayload::DuplicateId {
                    resource: StorageResource::ProductEntries,
                    id: core.owner.get(),
                }),
            });
        }
    }
    Ok(())
}

fn requirement_count(
    owners: &[OwnerExpressionSet],
    operation: FailureOperation,
) -> Result<usize, NecoFailure> {
    owners.iter().try_fold(0_usize, |total, core| {
        total.checked_add(core.requirements.len()).ok_or_else(|| {
            map_storage_failure(
                FailureLocation::operation(operation),
                crate::StorageError::CapacityOverflow {
                    resource: StorageResource::ProductEntries,
                },
            )
        })
    })
}

fn map_evaluation_run_failure(location: FailureLocation, error: EvaluationRunError) -> NecoFailure {
    match error {
        EvaluationRunError::UnknownExpr(expr) => NecoFailure::UnknownExpression {
            location: location.with_expr(expr),
        },
        EvaluationRunError::UnknownAtom { expr, atom } => NecoFailure::UnknownAtom {
            location: location.with_expr(expr).with_atom(atom),
        },
        EvaluationRunError::Evaluation(error) => map_eval_failure(location, error),
        EvaluationRunError::Storage(error) => map_storage_failure(location, error),
    }
}

fn map_float_failure(location: FailureLocation, error: PrecisionResolutionError) -> NecoFailure {
    match error {
        PrecisionResolutionError::Width { requested, width } => NecoFailure::PrecisionExhaustion {
            location,
            requested,
            payload: Box::new(PrecisionFailurePayload::EnclosureWidth {
                actual_width: width,
            }),
        },
        PrecisionResolutionError::WidthFailure { requested, error } => {
            NecoFailure::PrecisionExhaustion {
                location,
                requested,
                payload: Box::new(PrecisionFailurePayload::Bigint(error)),
            }
        }
        PrecisionResolutionError::Float(error) => match error {
            FloatError::OutOfRange => NecoFailure::FloatOutOfRange { location },
            FloatError::Bigint(error) => map_resolve_failure(location, ResolveError::Bigint(error)),
            FloatError::Algnum(error) => map_resolve_failure(location, ResolveError::Algnum(error)),
            FloatError::EnclosureBigint { requested, error } => {
                map_precision_bigint_failure(location, requested, error)
            }
            FloatError::EnclosureAlgnum { requested, error } => {
                map_precision_algnum_failure(location, requested, error)
            }
            FloatError::Storage(error) => map_storage_failure(location, error),
        },
    }
}

fn map_precision_bigint_failure(
    location: FailureLocation,
    requested: AbsoluteBits,
    error: BigintError,
) -> NecoFailure {
    match error {
        failure @ BigintError::ExponentOverflow { .. } => NecoFailure::PrecisionExhaustion {
            location,
            requested,
            payload: Box::new(PrecisionFailurePayload::Bigint(failure)),
        },
        error => map_resolve_failure(location, ResolveError::Bigint(error)),
    }
}

fn map_precision_algnum_failure(
    location: FailureLocation,
    requested: AbsoluteBits,
    error: AlgnumError,
) -> NecoFailure {
    match error {
        failure @ AlgnumError::RepresentationLimit { .. } => NecoFailure::PrecisionExhaustion {
            location,
            requested,
            payload: Box::new(PrecisionFailurePayload::Algnum(failure)),
        },
        AlgnumError::Bigint(failure @ BigintError::ExponentOverflow { .. }) => {
            NecoFailure::PrecisionExhaustion {
                location,
                requested,
                payload: Box::new(PrecisionFailurePayload::Bigint(failure)),
            }
        }
        error => map_resolve_failure(location, ResolveError::Algnum(error)),
    }
}

fn decision_operands(decision: ExactDecisionRequest) -> ([ExprId; 2], usize) {
    match decision {
        ExactDecisionRequest::Zero(expr)
        | ExactDecisionRequest::Sign(expr)
        | ExactDecisionRequest::Degeneracy(expr) => ([expr, expr], 1),
        ExactDecisionRequest::Equality(left, right) => ([left, right], 2),
    }
}

fn operation_name(operation: ExactOperation) -> &'static str {
    match operation {
        ExactOperation::NormalizeMonomial => "normalize-monomial",
        ExactOperation::NormalizeFormSum => "normalize-form-sum",
        ExactOperation::NormalizeAlgebraic => "normalize-algebraic",
        ExactOperation::DecideZero => "decide-zero",
        ExactOperation::DecideEquality => "decide-equality",
        ExactOperation::DecideSign => "decide-sign",
        ExactOperation::DecideDegeneracy => "decide-degeneracy",
        ExactOperation::ResolveCertifiedF64 => "resolve-certified-f64",
    }
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum InjectedOperationFailure {
    InvalidIsolation,
    MultipleTargetRoots,
    MissingAssignment(ConsumerId),
    FloatOutOfRange(ConsumerId, ExprId),
}

#[cfg(test)]
std::thread_local! {
    static INJECTED_OPERATION_FAILURE: core::cell::Cell<Option<InjectedOperationFailure>> = const { core::cell::Cell::new(None) };
}

#[cfg(test)]
fn injected_operation_failure(operation: FailureOperation) -> Option<NecoFailure> {
    INJECTED_OPERATION_FAILURE.with(|configured| {
        let failure = configured.take()?;
        let location = FailureLocation::operation(operation);
        Some(match failure {
            InjectedOperationFailure::InvalidIsolation => map_eval_failure(
                location,
                crate::EvalError::Algnum(neco_algnum::AlgnumError::InvalidIsolation),
            ),
            InjectedOperationFailure::MultipleTargetRoots => map_eval_failure(
                location,
                crate::EvalError::Algnum(neco_algnum::AlgnumError::MultipleTargetRoots),
            ),
            InjectedOperationFailure::MissingAssignment(consumer) => {
                map_resolve_failure(location, ResolveError::MissingAssignment { consumer })
            }
            InjectedOperationFailure::FloatOutOfRange(consumer, expr) => {
                map_resolve_failure(location, ResolveError::FloatOutOfRange { consumer, expr })
            }
        })
    })
}

#[cfg(test)]
fn with_injected_operation_failure<R>(
    failure: InjectedOperationFailure,
    operation: impl FnOnce() -> R,
) -> R {
    INJECTED_OPERATION_FAILURE.with(|configured| configured.set(Some(failure)));
    let result = operation();
    INJECTED_OPERATION_FAILURE.with(|configured| configured.set(None));
    result
}

#[cfg(test)]
mod proof_vectors {
    use alloc::vec;

    use neco_monomial::Monomial;

    use super::*;
    use crate::storage::{with_injected_failure, with_injected_failure_after, InjectedFailure};
    use crate::{AtomId, ExactValue, ExprNode, StorageError, StorageFailurePayload};

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

    fn domain(atom: Option<ExactValue>) -> (ExprGraph, AtomStore, ExprId) {
        let mut graph = ExprGraph::new();
        let expr = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let mut atoms = AtomStore::new();
        if let Some(value) = atom {
            atoms.insert(AtomId::new(0), value).unwrap();
        }
        (graph, atoms, expr)
    }

    fn requirement(consumer: u32, expr: ExprId) -> ExactExpressionRequirement {
        ExactExpressionRequirement::new(
            ConsumerId::new(consumer),
            &[expr],
            ExactDecisionRequest::Zero(expr),
            AbsoluteBits::new(20),
        )
        .unwrap()
    }

    fn input_with_domain(graph: ExprGraph, atoms: AtomStore, expr: ExprId) -> ExactAllocationInput {
        let first_owner =
            OwnerExpressionSet::new(OwnerId::new(1), graph, atoms, vec![requirement(0, expr)])
                .unwrap();
        let (second_owner_graph, second_owner_atoms, _) =
            domain(Some(ExactValue::Monomial(Monomial::one())));
        let second_owner = OwnerExpressionSet::new(
            OwnerId::new(2),
            second_owner_graph,
            second_owner_atoms,
            vec![],
        )
        .unwrap();
        ExactAllocationInput::new(
            vec![first_owner, second_owner],
            NecoObservedCapability::current(&OPERATIONS).unwrap(),
            NecoImplementationSource::current(),
        )
    }

    fn allocated_with_value(value: Option<ExactValue>) -> AllocatedExactNumericContext {
        let (graph, atoms, expr) = domain(value);
        allocate_exact_numeric(
            read_exact_numeric_inputs(input_with_domain(graph, atoms, expr)).unwrap(),
        )
        .unwrap()
    }

    fn normalized() -> NormalizedExactNumericContext {
        normalize_exact_expressions(allocated_with_value(Some(ExactValue::Monomial(
            Monomial::one(),
        ))))
        .unwrap()
    }

    fn decided() -> DecidedExactNumericContext {
        decide_exact_properties(normalized()).unwrap()
    }

    fn decided_with_two_consumers() -> DecidedExactNumericContext {
        let (graph, atoms, expr) = domain(Some(ExactValue::Monomial(Monomial::one())));
        let first_owner = OwnerExpressionSet::new(
            OwnerId::new(1),
            graph,
            atoms,
            vec![requirement(0, expr), requirement(1, expr)],
        )
        .unwrap();
        let (second_owner_graph, second_owner_atoms, _) =
            domain(Some(ExactValue::Monomial(Monomial::one())));
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
        decide_exact_properties(
            normalize_exact_expressions(
                allocate_exact_numeric(read_exact_numeric_inputs(input).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn decided_with_two_expressions() -> DecidedExactNumericContext {
        let mut graph = ExprGraph::new();
        let expr0 = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let expr1 = graph.push(ExprNode::Atom(AtomId::new(1))).unwrap();
        let mut atoms = AtomStore::new();
        atoms
            .insert(AtomId::new(0), ExactValue::Monomial(Monomial::zero()))
            .unwrap();
        atoms
            .insert(AtomId::new(1), ExactValue::Monomial(Monomial::one()))
            .unwrap();
        let requirement = ExactExpressionRequirement::new(
            ConsumerId::new(0),
            &[expr0, expr1],
            ExactDecisionRequest::Equality(expr0, expr1),
            AbsoluteBits::new(20),
        )
        .unwrap();
        let first_owner =
            OwnerExpressionSet::new(OwnerId::new(1), graph, atoms, vec![requirement]).unwrap();
        let (second_owner_graph, second_owner_atoms, _) =
            domain(Some(ExactValue::Monomial(Monomial::one())));
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
        decide_exact_properties(
            normalize_exact_expressions(
                allocate_exact_numeric(read_exact_numeric_inputs(input).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn generated_core(
        consumers: u32,
        offset: u32,
    ) -> (ExprGraph, AtomStore, Vec<ExactExpressionRequirement>) {
        let mut graph = ExprGraph::new();
        let mut atoms = AtomStore::new();
        atoms
            .insert(AtomId::new(0), ExactValue::Monomial(Monomial::one()))
            .unwrap();
        let unit = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let mut previous = unit;
        for _ in 0..offset {
            previous = graph.push(ExprNode::Add(previous, unit)).unwrap();
        }
        let mut exprs = Vec::new();
        for _ in 0..8 {
            previous = graph.push(ExprNode::Add(previous, unit)).unwrap();
            exprs.push(previous);
        }
        let mut requirements = Vec::new();
        for consumer in 0..consumers {
            let first = exprs[(consumer % 8) as usize];
            let second = exprs[((consumer * 5 + 3) % 8) as usize];
            let operands: &[ExprId] = if first == second {
                &[first]
            } else {
                &[second, first]
            };
            let decision = if first == second {
                ExactDecisionRequest::Zero(first)
            } else {
                ExactDecisionRequest::Equality(first, second)
            };
            requirements.push(
                ExactExpressionRequirement::new(
                    ConsumerId::new(consumer),
                    operands,
                    decision,
                    AbsoluteBits::new(10 + consumer % 2),
                )
                .unwrap(),
            );
        }
        (graph, atoms, requirements)
    }

    fn generated_input(consumers: u32) -> ExactAllocationInput {
        let (graph, atoms, requirements) = generated_core(consumers, 0);
        let first_owner =
            OwnerExpressionSet::new(OwnerId::new(1), graph, atoms, requirements).unwrap();
        let (graph, atoms, requirements) = generated_core(consumers, 20);
        let second_owner =
            OwnerExpressionSet::new(OwnerId::new(2), graph, atoms, requirements).unwrap();
        ExactAllocationInput::new(
            vec![first_owner, second_owner],
            NecoObservedCapability::current(&OPERATIONS).unwrap(),
            NecoImplementationSource::current(),
        )
    }

    fn generated_normalized(consumers: u32) -> NormalizedExactNumericContext {
        normalize_exact_expressions(
            allocate_exact_numeric(read_exact_numeric_inputs(generated_input(consumers)).unwrap())
                .unwrap(),
        )
        .unwrap()
    }

    fn generated_resolved(consumers: u32) -> ResolvedExactNumericContext {
        resolve_certified_f64(decide_exact_properties(generated_normalized(consumers)).unwrap())
            .unwrap()
    }

    fn shared_core(
        consumers: &[(u32, u32, u32)],
    ) -> (ExprGraph, AtomStore, Vec<ExactExpressionRequirement>) {
        let (graph, atoms, _) = generated_core(0, 0);
        let mut requirements = Vec::new();
        for (consumer, expr, bits) in consumers {
            let expr = ExprId::new(*expr);
            requirements.push(
                ExactExpressionRequirement::new(
                    ConsumerId::new(*consumer),
                    &[expr],
                    ExactDecisionRequest::Sign(expr),
                    AbsoluteBits::new(*bits),
                )
                .unwrap(),
            );
        }
        (graph, atoms, requirements)
    }

    fn shared_product(
        first_owner: &[(u32, u32, u32)],
        second_owner: &[(u32, u32, u32)],
    ) -> ExactComputationProduct {
        let (graph, atoms, requirements) = shared_core(first_owner);
        let first_owner =
            OwnerExpressionSet::new(OwnerId::new(1), graph, atoms, requirements).unwrap();
        let (graph, atoms, requirements) = shared_core(second_owner);
        let second_owner =
            OwnerExpressionSet::new(OwnerId::new(2), graph, atoms, requirements).unwrap();
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
        assemble_exact_computation_product(resolved).unwrap()
    }

    #[test]
    fn shared_values_match_single_consumer_references_and_share_storage() {
        let first_owner: Vec<(u32, u32, u32)> = (0..12_u32)
            .map(|consumer| (consumer, 5 + consumer % 3, 20 + 10 * (consumer % 2)))
            .collect();
        let second_owner: Vec<(u32, u32, u32)> = (0..6_u32)
            .map(|consumer| (consumer, 5 + consumer % 2, 20))
            .collect();
        let product = shared_product(&first_owner, &second_owner);
        let normalized = &product.resolved.decided.normalized;
        let owners = [
            (OwnerId::new(1), &first_owner),
            (OwnerId::new(2), &second_owner),
        ];
        for (owner, consumers) in owners {
            for (consumer, expr, bits) in consumers {
                let reference = if owner == OwnerId::new(1) {
                    shared_product(&[(*consumer, *expr, *bits)], &[])
                } else {
                    shared_product(&[], &[(*consumer, *expr, *bits)])
                };
                let consumer = ConsumerId::new(*consumer);
                let expr = ExprId::new(*expr);
                let value = normalized.normal_form(owner, consumer, expr).unwrap();
                let reference_normalized = &reference.resolved.decided.normalized;
                assert_eq!(
                    Some(value),
                    reference_normalized.normal_form(owner, consumer, expr)
                );
                let witness = product.resolved.decided.decision(owner, consumer).unwrap();
                let reference_witness = reference
                    .resolved
                    .decided
                    .decision(owner, consumer)
                    .unwrap();
                assert_eq!(witness, reference_witness);
                assert!(core::ptr::eq(witness.normal_form_witness(), value));
                let certified = &product
                    .resolved
                    .certified_descent(owner, consumer)
                    .unwrap()
                    .values[0];
                let reference_certified = &reference
                    .resolved
                    .certified_descent(owner, consumer)
                    .unwrap()
                    .values[0];
                assert_eq!(certified, reference_certified);
            }
        }
        for (owner, consumers) in owners {
            for (a, first) in consumers.iter().enumerate() {
                for second in &consumers[a + 1..] {
                    let first_consumer = ConsumerId::new(first.0);
                    let second_consumer = ConsumerId::new(second.0);
                    if first.1 == second.1 {
                        assert!(core::ptr::eq(
                            normalized
                                .normal_form(owner, first_consumer, ExprId::new(first.1))
                                .unwrap(),
                            normalized
                                .normal_form(owner, second_consumer, ExprId::new(second.1))
                                .unwrap(),
                        ));
                    }
                    let first_value = &product
                        .resolved
                        .certified_descent(owner, first_consumer)
                        .unwrap()
                        .values[0];
                    let second_value = &product
                        .resolved
                        .certified_descent(owner, second_consumer)
                        .unwrap()
                        .values[0];
                    let same = first.1 == second.1 && first.2 == second.2;
                    assert_eq!(
                        core::ptr::eq(first_value.certified(), second_value.certified()),
                        same
                    );
                }
            }
        }
        let shared_expr = ExprId::new(5);
        let first = normalized
            .normal_form(OwnerId::new(1), ConsumerId::new(0), shared_expr)
            .unwrap();
        let other = normalized
            .normal_form(OwnerId::new(2), ConsumerId::new(0), shared_expr)
            .unwrap();
        assert!(!core::ptr::eq(first, other));
        let bundle = |owner, consumer| {
            product
                .resolved
                .certified_descent(owner, ConsumerId::new(consumer))
                .unwrap()
                .values[0]
                .certified()
        };
        assert!(!core::ptr::eq(
            bundle(OwnerId::new(1), 0),
            bundle(OwnerId::new(2), 0)
        ));
        assert_eq!(product.resolved.shared_resolution_count(), 6 + 2);
    }

    #[test]
    fn keyed_lookups_match_linear_search_on_generated_input() {
        let resolved = generated_resolved(40);
        let normalized = &resolved.decided.normalized;
        let owners = [OwnerId::new(1), OwnerId::new(2)];
        for owner in owners {
            for consumer in 0..60_u32 {
                let consumer = ConsumerId::new(consumer);
                let expected = resolved
                    .certified_descents
                    .iter()
                    .find(|bundle| bundle.owner == owner && bundle.consumer_id == consumer);
                assert_eq!(resolved.certified_descent(owner, consumer), expected);
                let expected = resolved
                    .decided
                    .decisions
                    .iter()
                    .find(|entry| entry.owner == owner && entry.consumer == consumer)
                    .map(|entry| &entry.witness);
                assert_eq!(resolved.decided.decision(owner, consumer), expected);
                for expr in 0..40_u32 {
                    let expr = ExprId::new(expr);
                    let expected = normalized
                        .normal_forms
                        .iter()
                        .find(|entry| {
                            entry.owner == owner && entry.consumer == consumer && entry.expr == expr
                        })
                        .map(|entry| &*normalized.normal_values[entry.value_position]);
                    assert_eq!(normalized.normal_form(owner, consumer, expr), expected);
                }
            }
        }
        let first = ConsumerId::new(3);
        let expr = resolved.certified_descents[3].values[0].expr;
        assert_ne!(
            normalized.normal_form(OwnerId::new(1), first, expr),
            normalized.normal_form(OwnerId::new(2), first, expr)
        );
        let product = assemble_exact_computation_product(resolved).unwrap();
        let inspection = product.direct_inspection().unwrap();
        for owner in owners {
            for consumer in 0..60_u32 {
                let consumer = ConsumerId::new(consumer);
                let expected = product
                    .resolved
                    .decided
                    .decisions
                    .iter()
                    .find(|entry| entry.owner == owner && entry.consumer == consumer)
                    .map(|entry| &entry.witness);
                assert_eq!(inspection.decision(owner, consumer), expected);
            }
        }
    }

    fn typed_failures_until_success<T, R: core::fmt::Debug>(
        resource: StorageResource,
        operation: FailureOperation,
        setup: impl Fn() -> T,
        run: impl Fn(T) -> Result<R, NecoFailure>,
    ) -> usize {
        for skipped in 0..64 {
            let input = setup();
            let result =
                with_injected_failure_after(resource, InjectedFailure::Allocation, skipped, || {
                    run(input)
                });
            match result {
                Ok(_) => return skipped,
                Err(NecoFailure::StorageFailure { location, payload }) => {
                    assert_eq!(location.operation_kind(), operation);
                    assert!(matches!(
                        *payload,
                        StorageFailurePayload::Storage(StorageError::AllocationFailure {
                            resource: failed,
                            ..
                        }) if failed == resource
                    ));
                }
                Err(other) => panic!("unexpected failure: {other:?}"),
            }
        }
        panic!("operation kept failing");
    }

    #[test]
    fn index_allocation_failures_are_typed_storage_failures() {
        let allocated = || {
            allocate_exact_numeric(read_exact_numeric_inputs(generated_input(3)).unwrap()).unwrap()
        };
        let reserves = typed_failures_until_success(
            StorageResource::ProductEntries,
            FailureOperation::Normalize,
            allocated,
            normalize_exact_expressions,
        );
        assert_eq!(reserves, 39);
        let reserves = typed_failures_until_success(
            StorageResource::ProductEntries,
            FailureOperation::Decide,
            || generated_normalized(3),
            decide_exact_properties,
        );
        assert_eq!(reserves, 2 * 6 + 1);
        let decided = || decide_exact_properties(generated_normalized(1)).unwrap();
        let reserves = typed_failures_until_success(
            StorageResource::ProductEntries,
            FailureOperation::Resolve,
            decided,
            resolve_certified_f64,
        );
        assert_eq!(reserves, 3);
        let reserves = typed_failures_until_success(
            StorageResource::ProductResolutionEntries,
            FailureOperation::Resolve,
            decided,
            resolve_certified_f64,
        );
        assert!(reserves > 2);
    }

    #[test]
    fn work_area_allocation_failure_precedes_duplicate_failure() {
        let expr = ExprId::new(0);
        let duplicate = || {
            ExactExpressionRequirement::new(
                ConsumerId::new(0),
                &[expr, expr],
                ExactDecisionRequest::Zero(expr),
                AbsoluteBits::new(20),
            )
        };
        let plain = duplicate();
        assert!(matches!(
            plain,
            Err(NecoFailure::StorageFailure { location, payload })
                if location.expr() == Some(expr)
                    && matches!(
                        *payload,
                        StorageFailurePayload::DuplicateId { id: 0, .. }
                    )
        ));
        for skipped in 0..2 {
            let injected = with_injected_failure_after(
                StorageResource::ProductEntries,
                InjectedFailure::Allocation,
                skipped,
                duplicate,
            );
            assert!(matches!(
                injected,
                Err(NecoFailure::StorageFailure { payload, .. })
                    if matches!(
                        *payload,
                        StorageFailurePayload::Storage(StorageError::AllocationFailure {
                            resource: StorageResource::ProductEntries,
                            ..
                        })
                    )
            ));
        }
    }

    #[test]
    fn allocation_storage_failure_preserves_operation_location_and_payload() {
        let (graph, atoms, expr) = domain(Some(ExactValue::Monomial(Monomial::one())));
        let input = read_exact_numeric_inputs(input_with_domain(graph, atoms, expr)).unwrap();
        let result = with_injected_failure(
            StorageResource::ProductEntries,
            InjectedFailure::Allocation,
            || allocate_exact_numeric(input),
        );
        assert_eq!(
            result,
            Err(NecoFailure::StorageFailure {
                location: FailureLocation::operation(FailureOperation::Allocate),
                payload: Box::new(StorageFailurePayload::Storage(
                    StorageError::AllocationFailure {
                        resource: StorageResource::ProductEntries,
                        requested_elements: 1,
                    }
                )),
            })
        );
    }

    #[test]
    fn normalize_operation_failure_vectors_are_executable() {
        let (graph, atoms, expr) = domain(None);
        let unknown_atom = normalize_exact_expressions(
            allocate_exact_numeric(
                read_exact_numeric_inputs(input_with_domain(graph, atoms, expr)).unwrap(),
            )
            .unwrap(),
        );
        assert!(matches!(
            unknown_atom,
            Err(NecoFailure::UnknownAtom { location })
                if location.operation_kind() == FailureOperation::Normalize
                    && location.expr() == Some(expr)
                    && location.atom() == Some(AtomId::new(0))
        ));

        let mut malformed = allocated_with_value(Some(ExactValue::Monomial(Monomial::one())));
        malformed.input.owners[0].requirements[0].expressions[0] = ExprId::new(99);
        assert!(matches!(
            normalize_exact_expressions(malformed),
            Err(NecoFailure::UnknownExpression { location })
                if location.operation_kind() == FailureOperation::Normalize
                    && location.expr() == Some(ExprId::new(99))
        ));

        let mut graph = ExprGraph::new();
        let zero = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let division = graph.push(ExprNode::Div(zero, zero)).unwrap();
        let mut atoms = AtomStore::new();
        atoms
            .insert(AtomId::new(0), ExactValue::Monomial(Monomial::zero()))
            .unwrap();
        assert!(matches!(
            normalize_exact_expressions(
                allocate_exact_numeric(
                    read_exact_numeric_inputs(input_with_domain(graph, atoms, division)).unwrap(),
                )
                .unwrap(),
            ),
            Err(NecoFailure::DivisionByZero { location })
                if location.operation_kind() == FailureOperation::Normalize
        ));

        let mut graph = ExprGraph::new();
        let zero = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let power = graph
            .push(ExprNode::Pow {
                base: zero,
                exponent: neco_bigint::ReducedRational::from_bigint(neco_bigint::BigInt::zero())
                    .unwrap(),
            })
            .unwrap();
        let mut atoms = AtomStore::new();
        atoms
            .insert(AtomId::new(0), ExactValue::Monomial(Monomial::zero()))
            .unwrap();
        assert!(matches!(
            normalize_exact_expressions(
                allocate_exact_numeric(
                    read_exact_numeric_inputs(input_with_domain(graph, atoms, power)).unwrap(),
                )
                .unwrap(),
            ),
            Err(NecoFailure::UndefinedPower { location })
                if location.operation_kind() == FailureOperation::Normalize
        ));

        let invalid_isolation =
            with_injected_operation_failure(InjectedOperationFailure::InvalidIsolation, || {
                normalize_exact_expressions(allocated_with_value(Some(ExactValue::Monomial(
                    Monomial::one(),
                ))))
            });
        assert!(matches!(
            invalid_isolation,
            Err(NecoFailure::InvalidIsolation { location })
                if location.operation_kind() == FailureOperation::Normalize
        ));

        let storage = with_injected_failure(
            StorageResource::EvaluationEntries,
            InjectedFailure::Allocation,
            || {
                normalize_exact_expressions(allocated_with_value(Some(ExactValue::Monomial(
                    Monomial::one(),
                ))))
            },
        );
        assert!(matches!(
            storage,
            Err(NecoFailure::StorageFailure { location, payload })
                if location.operation_kind() == FailureOperation::Normalize
                    && matches!(
                        *payload,
                        StorageFailurePayload::Storage(StorageError::AllocationFailure {
                            resource: StorageResource::EvaluationEntries,
                            ..
                        })
                    )
        ));
    }

    #[test]
    fn decide_operation_failure_vectors_are_executable() {
        let invalid_input = normalized();
        let invalid =
            with_injected_operation_failure(InjectedOperationFailure::InvalidIsolation, || {
                decide_exact_properties(invalid_input)
            });
        assert!(matches!(
            invalid,
            Err(NecoFailure::InvalidIsolation { location })
                if location.operation_kind() == FailureOperation::Decide
        ));
        let multiple_input = normalized();
        let multiple =
            with_injected_operation_failure(InjectedOperationFailure::MultipleTargetRoots, || {
                decide_exact_properties(multiple_input)
            });
        assert!(matches!(
            multiple,
            Err(NecoFailure::MultipleTargetRoots { location })
                if location.operation_kind() == FailureOperation::Decide
        ));
        let storage_input = normalized();
        let storage = with_injected_failure(
            StorageResource::ProductEntries,
            InjectedFailure::Allocation,
            || decide_exact_properties(storage_input),
        );
        assert!(matches!(
            storage,
            Err(NecoFailure::StorageFailure { location, payload })
                if location.operation_kind() == FailureOperation::Decide
                    && matches!(
                        *payload,
                        StorageFailurePayload::Storage(StorageError::AllocationFailure {
                            resource: StorageResource::ProductEntries,
                            ..
                        })
                    )
        ));
    }

    #[test]
    fn resolve_operation_failure_vectors_are_executable() {
        let consumer = ConsumerId::new(7);
        let missing_input = decided();
        let missing = with_injected_operation_failure(
            InjectedOperationFailure::MissingAssignment(consumer),
            || resolve_certified_f64(missing_input),
        );
        assert!(matches!(
            missing,
            Err(NecoFailure::MissingAssignment { location })
                if location.operation_kind() == FailureOperation::Resolve
                    && location.consumer() == Some(consumer)
        ));
        let expr = ExprId::new(3);
        let out_of_range_input = decided();
        let out_of_range = with_injected_operation_failure(
            InjectedOperationFailure::FloatOutOfRange(consumer, expr),
            || resolve_certified_f64(out_of_range_input),
        );
        assert!(matches!(
            out_of_range,
            Err(NecoFailure::FloatOutOfRange { location })
                if location.operation_kind() == FailureOperation::Resolve
                    && location.consumer() == Some(consumer)
                    && location.expr() == Some(expr)
        ));
        let storage = with_injected_failure(
            StorageResource::ProductResolutionEntries,
            InjectedFailure::Allocation,
            || resolve_certified_f64(decided()),
        );
        assert!(matches!(
            storage,
            Err(NecoFailure::StorageFailure { location, payload })
                if location.operation_kind() == FailureOperation::Resolve
                    && matches!(
                        *payload,
                        StorageFailurePayload::Storage(StorageError::AllocationFailure {
                            resource: StorageResource::ProductResolutionEntries,
                            ..
                        })
                    )
        ));
    }

    #[test]
    fn missing_domain_assembly_returns_failure_without_partial_product() {
        let mut resolved = resolve_certified_f64(decided()).unwrap();
        resolved.certified_descents.clear();
        let result = assemble_exact_computation_product(resolved);
        assert!(matches!(
            result,
            Err(NecoFailure::MissingAssignment { location })
                if location.operation_kind() == FailureOperation::Assemble
        ));
    }

    fn reindex(resolved: &mut ResolvedExactNumericContext) {
        resolved.descent_index = SortedIndex::build(
            resolved
                .certified_descents
                .iter()
                .map(|bundle| (bundle.owner, bundle.consumer_id)),
            StorageResource::ProductEntries,
        )
        .unwrap();
        for bundle in &mut resolved.certified_descents {
            bundle.value_index = bundle_value_index(
                &bundle.values,
                FailureLocation::operation(FailureOperation::Assemble),
            )
            .unwrap();
        }
    }

    fn assembly_mismatch_field(
        mut resolved: ResolvedExactNumericContext,
        mutate: impl FnOnce(&mut CertifiedF64Bundle),
    ) -> ProvenanceMismatchField {
        mutate(&mut resolved.certified_descents[0]);
        reindex(&mut resolved);
        match assemble_exact_computation_product(resolved) {
            Err(NecoFailure::ProvenanceMismatch { payload, .. }) => payload.field(),
            result => panic!("unexpected assembly result: {result:?}"),
        }
    }

    fn assembly_mismatch_payload(
        mut resolved: ResolvedExactNumericContext,
        mutate: impl FnOnce(&mut CertifiedF64Bundle),
    ) -> ProvenanceMismatchPayload {
        mutate(&mut resolved.certified_descents[0]);
        reindex(&mut resolved);
        match assemble_exact_computation_product(resolved) {
            Err(NecoFailure::ProvenanceMismatch { payload, .. }) => *payload,
            result => panic!("unexpected assembly result: {result:?}"),
        }
    }

    #[test]
    fn assembly_rejects_each_result_lineage_mismatch() {
        assert_eq!(
            assembly_mismatch_field(
                resolve_certified_f64(decided_with_two_consumers()).unwrap(),
                |bundle| {
                    bundle.consumer_id = ConsumerId::new(1);
                },
            ),
            ProvenanceMismatchField::Consumer
        );
        assert_eq!(
            assembly_mismatch_field(
                resolve_certified_f64(decided_with_two_expressions()).unwrap(),
                |bundle| {
                    bundle.values[0].expr = ExprId::new(1);
                },
            ),
            ProvenanceMismatchField::Expression
        );
        assert_eq!(
            assembly_mismatch_field(resolve_certified_f64(decided()).unwrap(), |bundle| {
                bundle.consumer_id = ConsumerId::new(9);
            }),
            ProvenanceMismatchField::Consumer
        );
        assert_eq!(
            assembly_mismatch_field(resolve_certified_f64(decided()).unwrap(), |bundle| {
                bundle.owner = OwnerId::new(2);
            }),
            ProvenanceMismatchField::Owner
        );
        assert_eq!(
            assembly_mismatch_field(resolve_certified_f64(decided()).unwrap(), |bundle| {
                bundle.precision = AbsoluteBits::new(99);
            }),
            ProvenanceMismatchField::Precision
        );
        assert_eq!(
            assembly_mismatch_field(resolve_certified_f64(decided()).unwrap(), |bundle| {
                bundle.source = NecoImplementationSource::new("wrong", "wrong-api");
            }),
            ProvenanceMismatchField::Source
        );
        assert_eq!(
            assembly_mismatch_field(resolve_certified_f64(decided()).unwrap(), |bundle| {
                bundle.values[0].expr = ExprId::new(99);
            }),
            ProvenanceMismatchField::Expression
        );
    }

    fn lineage_with_owner(owner: OwnerId) -> ProvenanceRecord {
        ProvenanceRecord::new(
            Some(owner),
            Some(ConsumerId::new(3)),
            Some(ExprId::new(4)),
            Some(AbsoluteBits::new(20)),
            Some("revision"),
            Some("api"),
        )
    }

    #[test]
    fn lineage_comparison_distinguishes_owners_by_identifier_value_alone() {
        let location = FailureLocation::operation(FailureOperation::Assemble);
        for owner_value in [0, 1, 7, u32::MAX] {
            assert!(compare_provenance(
                location,
                lineage_with_owner(OwnerId::new(owner_value)),
                lineage_with_owner(OwnerId::new(owner_value)),
            )
            .is_ok());
        }
        for (expected_value, actual_value) in [(0, 1), (1, 0), (7, 8), (1, u32::MAX)] {
            let expected = lineage_with_owner(OwnerId::new(expected_value));
            let actual = lineage_with_owner(OwnerId::new(actual_value));
            match compare_provenance(location, expected, actual) {
                Err(NecoFailure::ProvenanceMismatch { payload, .. }) => {
                    assert_eq!(payload.field(), ProvenanceMismatchField::Owner);
                    assert_eq!(
                        payload.expected().owner(),
                        Some(OwnerId::new(expected_value))
                    );
                    assert_eq!(payload.actual().owner(), Some(OwnerId::new(actual_value)));
                }
                result => panic!("unexpected comparison result: {result:?}"),
            }
        }
    }

    fn expected_expression_provenance() -> ProvenanceRecord {
        ProvenanceRecord::new(
            Some(OwnerId::new(1)),
            Some(ConsumerId::new(0)),
            Some(ExprId::new(0)),
            Some(AbsoluteBits::new(20)),
            Some(env!("CARGO_PKG_VERSION")),
            Some(PUBLIC_API_IDENTITY),
        )
    }

    fn assert_nested_lineage_payload(
        payload: ProvenanceMismatchPayload,
        field: ProvenanceMismatchField,
        actual: ProvenanceRecord,
    ) {
        assert_eq!(payload.field(), field);
        assert_eq!(payload.expected(), expected_expression_provenance());
        assert_eq!(payload.actual(), actual);
    }

    #[test]
    fn nested_expression_lineage_failure_contains_expected_and_actual_records() {
        let payload =
            assembly_mismatch_payload(resolve_certified_f64(decided()).unwrap(), |bundle| {
                bundle.values[0].owner = OwnerId::new(2);
            });
        assert_nested_lineage_payload(
            payload,
            ProvenanceMismatchField::Owner,
            ProvenanceRecord::new(
                Some(OwnerId::new(2)),
                Some(ConsumerId::new(0)),
                Some(ExprId::new(0)),
                Some(AbsoluteBits::new(20)),
                Some(env!("CARGO_PKG_VERSION")),
                Some(PUBLIC_API_IDENTITY),
            ),
        );

        let payload =
            assembly_mismatch_payload(resolve_certified_f64(decided()).unwrap(), |bundle| {
                bundle.values[0].consumer_id = ConsumerId::new(99);
            });
        assert_nested_lineage_payload(
            payload,
            ProvenanceMismatchField::Consumer,
            ProvenanceRecord::new(
                Some(OwnerId::new(1)),
                Some(ConsumerId::new(99)),
                Some(ExprId::new(0)),
                Some(AbsoluteBits::new(20)),
                Some(env!("CARGO_PKG_VERSION")),
                Some(PUBLIC_API_IDENTITY),
            ),
        );

        let payload =
            assembly_mismatch_payload(resolve_certified_f64(decided()).unwrap(), |bundle| {
                bundle.values[0].precision = AbsoluteBits::new(99);
            });
        assert_nested_lineage_payload(
            payload,
            ProvenanceMismatchField::Precision,
            ProvenanceRecord::new(
                Some(OwnerId::new(1)),
                Some(ConsumerId::new(0)),
                Some(ExprId::new(0)),
                Some(AbsoluteBits::new(99)),
                Some(env!("CARGO_PKG_VERSION")),
                Some(PUBLIC_API_IDENTITY),
            ),
        );

        let payload =
            assembly_mismatch_payload(resolve_certified_f64(decided()).unwrap(), |bundle| {
                bundle.values[0].source = NecoImplementationSource::new("wrong", "wrong-api");
            });
        assert_nested_lineage_payload(
            payload,
            ProvenanceMismatchField::Source,
            ProvenanceRecord::new(
                Some(OwnerId::new(1)),
                Some(ConsumerId::new(0)),
                Some(ExprId::new(0)),
                Some(AbsoluteBits::new(20)),
                Some("wrong"),
                Some("wrong-api"),
            ),
        );
    }

    #[test]
    fn representation_limit_maps_to_typed_precision_exhaustion() {
        let requested = AbsoluteBits::new(80);
        let location = FailureLocation::operation(FailureOperation::Resolve)
            .with_consumer(ConsumerId::new(0))
            .with_expr(ExprId::new(0));
        let result = map_float_failure(
            location,
            PrecisionResolutionError::Float(FloatError::EnclosureAlgnum {
                requested,
                error: AlgnumError::RepresentationLimit {
                    resource: neco_algnum::RepresentationResource::RootDegree,
                    required: neco_bigint::BigUint::try_from(41_u8).unwrap(),
                    maximum: neco_bigint::BigUint::try_from(40_u8).unwrap(),
                },
            }),
        );
        assert!(matches!(
            result,
            NecoFailure::PrecisionExhaustion {
                location: actual,
                requested: actual_requested,
                payload,
            } if actual == location
                && actual_requested == requested
                && matches!(
                    *payload,
                    PrecisionFailurePayload::Algnum(AlgnumError::RepresentationLimit { .. })
                )
        ));
    }
}
