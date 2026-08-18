use alloc::vec::Vec;

use neco_bigint::Sign;

use crate::error::{map_eval_failure, map_resolve_failure, map_storage_failure};
use crate::evaluate::{evaluate_reachable, EvaluationRunError};
use crate::float::FloatError;
use crate::storage::{reserve_entries, StorageResource};
use crate::value::{decide_equality, decide_sign, decide_zero};
use crate::{
    AbsoluteBits, AtomStore, CertifiedF64, ConsumerId, EvaluationCache, ExactValue, ExprGraph,
    ExprId, FailureLocation, FailureOperation, IsolationCache, NecoFailure, ResolveError,
    UnsupportedFailurePayload,
};

pub const IMPLEMENTATION_REVISION: &str = "64f179e915c48a32c9652c1be0a91748045dfd4d";
pub const PUBLIC_API_IDENTITY: &str = "neco-expr-exact-computation-v1";

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
pub enum NumericalOwner {
    ModalFieldProjection,
    Wavesim,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactInput {
    GeometryIdentity,
    GeometryDimension,
    SourceNode,
    ReceiverNode,
    FrequencyBandEndpoints,
    ModeLimit,
    SamplingCount,
    SamplingRate,
    ModeIndexOrdering,
    ModeRowCardinality,
    ModeRowWidth,
    DampingDefinition,
    SystemIdentity,
    SubsystemIdentity,
    StateShape,
    StateExtents,
    InitialStateIndex,
    TimeDomainEndpoints,
    CalibrationInterval,
    HeldOutInterval,
    ConditionIdentity,
    CouplingTopology,
    CouplingSelector,
    ComparatorDirection,
    AcceptanceDomainBound,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExactDecisionAssignment {
    ProvenanceNonempty,
    SourceReceiverIdentityValidity,
    ModeSetNonempty,
    ModeIdentityEqualityOrdering,
    ModeShapeCardinalityAxisEquality,
    SamplingDomainValidity,
    ZeroDivisionGuard,
    Rt60RoundTripEquality,
    MfpBranchIdentity,
    StateShapeEquality,
    CalibrationHeldOutDisjointness,
    ConditionSetCompleteness,
    SubsystemIdentityEquality,
    FiniteNonNegative,
    EnergyBalance,
    SingularityZeroDenominator,
    PredictionIndependenceEquality,
    R2LowerBound,
    AssemblyFailureSetEmptiness,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NumericalOperation {
    ModeFrequencyEvaluation,
    ModeShapeEvaluation,
    DampingRateCalculation,
    ModeContributionCalculation,
    ModeSum,
    ReceivedSeriesSamplingAccumulation,
    TranscendentalEvaluationFem,
    ModalRhsEvaluation,
    InitialStateNumericalConstruction,
    OdeIntegration,
    EnergyObservation,
    DddRegressionEstimation,
    SeaRegressionEstimation,
    HeldOutPredictionComparison,
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NumericalBudgetComponent {
    ModeTruncation,
    ModeShapeEvaluation,
    DampingRt60Conversion,
    ModeSumAccumulation,
    ReceivedSeriesSamplingAccumulation,
    SolverTolerance,
    MaximumStepDiscretization,
    OdeIntegration,
    EnergyBalanceTolerance,
    SmoothingRequirement,
    DddRegressionEstimation,
    SeaRegressionEstimation,
    HeldOutComparatorTolerance,
    R2AcceptanceMargin,
}

#[derive(Debug, Eq, PartialEq)]
pub struct NumericalErrorBudget {
    owner: NumericalOwner,
    consumer_id: ConsumerId,
    component: NumericalBudgetComponent,
    requirement_or_bound: AbsoluteBits,
}

impl NumericalErrorBudget {
    pub const fn new(
        owner: NumericalOwner,
        consumer_id: ConsumerId,
        component: NumericalBudgetComponent,
        requirement_or_bound: AbsoluteBits,
    ) -> Self {
        Self {
            owner,
            consumer_id,
            component,
            requirement_or_bound,
        }
    }

    pub const fn owner(&self) -> NumericalOwner {
        self.owner
    }

    pub const fn consumer_id(&self) -> ConsumerId {
        self.consumer_id
    }

    pub const fn component(&self) -> NumericalBudgetComponent {
        self.component
    }

    pub const fn requirement_or_bound(&self) -> AbsoluteBits {
        self.requirement_or_bound
    }
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
        for expression in expressions {
            if owned.binary_search(expression).is_ok() {
                continue;
            }
            let index = owned
                .binary_search(expression)
                .unwrap_or_else(|index| index);
            owned.insert(index, *expression);
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
pub struct MfpCoreProduct {
    graph: ExprGraph,
    atoms: AtomStore,
    requirements: Vec<ExactExpressionRequirement>,
    numerical_error_budgets: Vec<NumericalErrorBudget>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct WavesimDddCoreProduct {
    graph: ExprGraph,
    atoms: AtomStore,
    requirements: Vec<ExactExpressionRequirement>,
    numerical_error_budgets: Vec<NumericalErrorBudget>,
}

impl MfpCoreProduct {
    pub fn new(
        graph: ExprGraph,
        atoms: AtomStore,
        requirements: Vec<ExactExpressionRequirement>,
        numerical_error_budgets: Vec<NumericalErrorBudget>,
    ) -> Result<Self, NecoFailure> {
        validate_core(
            NumericalOwner::ModalFieldProjection,
            &graph,
            &requirements,
            &numerical_error_budgets,
        )?;
        Ok(Self {
            graph,
            atoms,
            requirements,
            numerical_error_budgets,
        })
    }

    pub fn requirements(&self) -> &[ExactExpressionRequirement] {
        &self.requirements
    }

    pub fn numerical_error_budgets(&self) -> &[NumericalErrorBudget] {
        &self.numerical_error_budgets
    }
}

impl WavesimDddCoreProduct {
    pub fn new(
        graph: ExprGraph,
        atoms: AtomStore,
        requirements: Vec<ExactExpressionRequirement>,
        numerical_error_budgets: Vec<NumericalErrorBudget>,
    ) -> Result<Self, NecoFailure> {
        validate_core(
            NumericalOwner::Wavesim,
            &graph,
            &requirements,
            &numerical_error_budgets,
        )?;
        Ok(Self {
            graph,
            atoms,
            requirements,
            numerical_error_budgets,
        })
    }

    pub fn requirements(&self) -> &[ExactExpressionRequirement] {
        &self.requirements
    }

    pub fn numerical_error_budgets(&self) -> &[NumericalErrorBudget] {
        &self.numerical_error_budgets
    }
}

fn validate_core(
    owner: NumericalOwner,
    graph: &ExprGraph,
    requirements: &[ExactExpressionRequirement],
    budgets: &[NumericalErrorBudget],
) -> Result<(), NecoFailure> {
    for requirement in requirements {
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
    for budget in budgets {
        if budget.owner() != owner
            || !requirements
                .iter()
                .any(|requirement| requirement.consumer_id() == budget.consumer_id())
        {
            return Err(NecoFailure::MissingAssignment {
                location: FailureLocation::operation(FailureOperation::Allocate)
                    .with_consumer(budget.consumer_id()),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
pub struct NecoObservedCapability {
    source_identity: &'static str,
    supported_operations: Vec<ExactOperation>,
}

impl NecoObservedCapability {
    pub fn current(supported_operations: &[ExactOperation]) -> Result<Self, NecoFailure> {
        Self::new(IMPLEMENTATION_REVISION, supported_operations)
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

#[derive(Debug, Eq, PartialEq)]
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
        Self::new(IMPLEMENTATION_REVISION, PUBLIC_API_IDENTITY)
    }

    pub const fn revision_identity(&self) -> &'static str {
        self.revision_identity
    }

    pub const fn public_api_identity(&self) -> &'static str {
        self.public_api_identity
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ExactAllocationInput {
    mfp_core: MfpCoreProduct,
    wavesim_ddd_core: WavesimDddCoreProduct,
    observed_capability: NecoObservedCapability,
    implementation_source: NecoImplementationSource,
}

impl ExactAllocationInput {
    pub const fn new(
        mfp_core: MfpCoreProduct,
        wavesim_ddd_core: WavesimDddCoreProduct,
        observed_capability: NecoObservedCapability,
        implementation_source: NecoImplementationSource,
    ) -> Self {
        Self {
            mfp_core,
            wavesim_ddd_core,
            observed_capability,
            implementation_source,
        }
    }

    pub fn mfp_core(&self) -> &MfpCoreProduct {
        &self.mfp_core
    }

    pub fn wavesim_ddd_core(&self) -> &WavesimDddCoreProduct {
        &self.wavesim_ddd_core
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
    exact_inputs: Vec<ExactInput>,
    exact_decisions: Vec<ExactDecisionAssignment>,
    numerical_operations: Vec<NumericalOperation>,
    descent_consumers: Vec<ConsumerId>,
    numerical_error_budgets: Vec<NumericalErrorBudget>,
}

impl ExactNumericAllocation {
    pub fn exact_inputs(&self) -> &[ExactInput] {
        &self.exact_inputs
    }

    pub fn exact_decisions(&self) -> &[ExactDecisionAssignment] {
        &self.exact_decisions
    }

    pub fn numerical_operations(&self) -> &[NumericalOperation] {
        &self.numerical_operations
    }

    pub fn descent_consumers(&self) -> &[ConsumerId] {
        &self.descent_consumers
    }

    pub fn numerical_error_budgets(&self) -> &[NumericalErrorBudget] {
        &self.numerical_error_budgets
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
    owner: NumericalOwner,
    consumer: ConsumerId,
    expr: ExprId,
    value: ExactValue,
}

#[derive(Debug, Eq, PartialEq)]
pub struct NormalizedExactNumericContext {
    allocated: AllocatedExactNumericContext,
    normal_forms: Vec<NormalFormEntry>,
    evaluation_count: usize,
}

impl NormalizedExactNumericContext {
    pub fn normal_form(&self, consumer: ConsumerId, expr: ExprId) -> Option<&ExactValue> {
        self.normal_forms
            .iter()
            .find(|entry| entry.consumer == consumer && entry.expr == expr)
            .map(|entry| &entry.value)
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
    normal_form_witness: ExactValue,
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
    consumer: ConsumerId,
    witness: ExactDecisionWitness,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DecidedExactNumericContext {
    normalized: NormalizedExactNumericContext,
    decisions: Vec<DecisionEntry>,
}

impl DecidedExactNumericContext {
    pub fn decision(&self, consumer: ConsumerId) -> Option<&ExactDecisionWitness> {
        self.decisions
            .iter()
            .find(|entry| entry.consumer == consumer)
            .map(|entry| &entry.witness)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct CertifiedF64Expression {
    expr: ExprId,
    certified: CertifiedF64,
}

impl CertifiedF64Expression {
    pub const fn expr(&self) -> ExprId {
        self.expr
    }

    pub fn certified(&self) -> &CertifiedF64 {
        &self.certified
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct CertifiedF64Bundle {
    consumer_id: ConsumerId,
    values: Vec<CertifiedF64Expression>,
}

struct CachedResolution {
    owner: NumericalOwner,
    expr: ExprId,
    bits: AbsoluteBits,
    value: CertifiedF64,
}

impl CertifiedF64Bundle {
    pub const fn consumer_id(&self) -> ConsumerId {
        self.consumer_id
    }

    pub fn values(&self) -> &[CertifiedF64Expression] {
        &self.values
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ResolvedExactNumericContext {
    decided: DecidedExactNumericContext,
    certified_descents: Vec<CertifiedF64Bundle>,
    shared_resolution_count: usize,
}

impl ResolvedExactNumericContext {
    pub fn certified_descent(&self, consumer: ConsumerId) -> Option<&CertifiedF64Bundle> {
        self.certified_descents
            .iter()
            .find(|bundle| bundle.consumer_id == consumer)
    }

    pub const fn shared_resolution_count(&self) -> usize {
        self.shared_resolution_count
    }
}

pub struct DirectInspection<'a> {
    allocation: &'a ExactNumericAllocation,
    requirements: Vec<&'a ExactExpressionRequirement>,
    decisions: &'a [DecisionEntry],
    certified_descents: &'a [CertifiedF64Bundle],
    shared_resolution_count: usize,
}

impl DirectInspection<'_> {
    pub fn allocation(&self) -> &ExactNumericAllocation {
        self.allocation
    }

    pub fn requirements(&self) -> &[&ExactExpressionRequirement] {
        &self.requirements
    }

    pub fn certified_descents(&self) -> &[CertifiedF64Bundle] {
        self.certified_descents
    }

    pub const fn shared_resolution_count(&self) -> usize {
        self.shared_resolution_count
    }

    pub fn decision(&self, consumer: ConsumerId) -> Option<&ExactDecisionWitness> {
        self.decisions
            .iter()
            .find(|entry| entry.consumer == consumer)
            .map(|entry| &entry.witness)
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

    pub fn normal_form(&self, consumer: ConsumerId, expr: ExprId) -> Option<&ExactValue> {
        self.resolved.decided.normalized.normal_form(consumer, expr)
    }

    pub fn decision(&self, consumer: ConsumerId) -> Option<&ExactDecisionWitness> {
        self.resolved.decided.decision(consumer)
    }

    pub fn certified_descent(&self, consumer: ConsumerId) -> Option<&CertifiedF64Bundle> {
        self.resolved.certified_descent(consumer)
    }

    pub fn direct_inspection(&self) -> Result<DirectInspection<'_>, NecoFailure> {
        let mfp = &self.resolved.decided.normalized.allocated.input.mfp_core;
        let wavesim = &self
            .resolved
            .decided
            .normalized
            .allocated
            .input
            .wavesim_ddd_core;
        let total = mfp
            .requirements
            .len()
            .checked_add(wavesim.requirements.len())
            .ok_or_else(|| {
                map_storage_failure(
                    FailureLocation::operation(FailureOperation::Assemble),
                    crate::StorageError::CapacityOverflow {
                        resource: StorageResource::ProductEntries,
                    },
                )
            })?;
        let mut requirements = Vec::new();
        reserve_entries(&mut requirements, total, StorageResource::ProductEntries).map_err(
            |error| {
                map_storage_failure(
                    FailureLocation::operation(FailureOperation::Assemble),
                    error,
                )
            },
        )?;
        requirements.extend(mfp.requirements.iter());
        requirements.extend(wavesim.requirements.iter());
        Ok(DirectInspection {
            allocation: self.allocation(),
            requirements,
            decisions: &self.resolved.decided.decisions,
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
    if input.implementation_source.revision_identity != IMPLEMENTATION_REVISION
        || input.implementation_source.public_api_identity != PUBLIC_API_IDENTITY
        || input.observed_capability.source_identity != IMPLEMENTATION_REVISION
    {
        return Err(NecoFailure::UnsupportedRequiredOperation {
            location: FailureLocation::operation(FailureOperation::Read),
            payload: UnsupportedFailurePayload::SourceIdentity,
        });
    }
    Ok(input)
}

pub fn allocate_exact_numeric(
    mut input: ExactAllocationInput,
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
                payload: UnsupportedFailurePayload::RequiredOperation(operation_name(operation)),
            });
        }
    }
    let mut consumers = Vec::new();
    let mut budgets = Vec::new();
    for requirement in input
        .mfp_core
        .requirements
        .iter()
        .chain(input.wavesim_ddd_core.requirements.iter())
    {
        if consumers.binary_search(&requirement.consumer_id).is_ok() {
            return Err(NecoFailure::StorageFailure {
                location: FailureLocation::operation(FailureOperation::Allocate)
                    .with_consumer(requirement.consumer_id),
                payload: crate::StorageFailurePayload::DuplicateId {
                    resource: StorageResource::ProductEntries,
                    id: requirement.consumer_id.get(),
                },
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
        reserve_entries(&mut consumers, required, StorageResource::ProductEntries).map_err(
            |error| {
                map_storage_failure(
                    FailureLocation::operation(FailureOperation::Allocate),
                    error,
                )
            },
        )?;
        let index = consumers
            .binary_search(&requirement.consumer_id)
            .unwrap_or_else(|index| index);
        consumers.insert(index, requirement.consumer_id);
    }
    move_budgets(&mut budgets, &mut input.mfp_core.numerical_error_budgets)?;
    move_budgets(
        &mut budgets,
        &mut input.wavesim_ddd_core.numerical_error_budgets,
    )?;
    let allocation = ExactNumericAllocation {
        exact_inputs: all_exact_inputs(),
        exact_decisions: all_exact_decisions(),
        numerical_operations: all_numerical_operations(),
        descent_consumers: consumers,
        numerical_error_budgets: budgets,
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
    let mut normal_forms = Vec::new();
    let mut evaluation_count = 0_usize;
    normalize_core(
        NumericalOwner::ModalFieldProjection,
        &allocated.input.mfp_core,
        &mut normal_forms,
        &mut evaluation_count,
    )?;
    normalize_core(
        NumericalOwner::Wavesim,
        &allocated.input.wavesim_ddd_core,
        &mut normal_forms,
        &mut evaluation_count,
    )?;
    Ok(NormalizedExactNumericContext {
        allocated,
        normal_forms,
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
    decide_core(
        &normalized,
        &normalized.allocated.input.mfp_core.requirements,
        &mut decisions,
    )?;
    decide_core(
        &normalized,
        &normalized.allocated.input.wavesim_ddd_core.requirements,
        &mut decisions,
    )?;
    Ok(DecidedExactNumericContext {
        normalized,
        decisions,
    })
}

fn decide_core(
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
        let first = normalized
            .normal_form(requirement.consumer_id, operands[0])
            .ok_or(NecoFailure::UnknownExpression {
                location: location.with_expr(operands[0]),
            })?;
        let (value, witness) = match requirement.decision {
            ExactDecisionRequest::Zero(_) => (
                if decide_zero(first) {
                    ExactDecisionValue::Zero
                } else {
                    ExactDecisionValue::NonZero
                },
                first
                    .try_clone()
                    .map_err(|error| map_eval_failure(location, error))?,
            ),
            ExactDecisionRequest::Equality(_, right) => {
                let right_value = normalized
                    .normal_form(requirement.consumer_id, right)
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
                    difference,
                )
            }
            ExactDecisionRequest::Sign(_) => (
                match decide_sign(first).map_err(|error| map_eval_failure(location, error))? {
                    Sign::Negative => ExactDecisionValue::Negative,
                    Sign::Zero => ExactDecisionValue::Zero,
                    Sign::Positive => ExactDecisionValue::Positive,
                },
                first
                    .try_clone()
                    .map_err(|error| map_eval_failure(location, error))?,
            ),
            ExactDecisionRequest::Degeneracy(_) => (
                if decide_zero(first) {
                    ExactDecisionValue::Degenerate
                } else {
                    ExactDecisionValue::NonDegenerate
                },
                first
                    .try_clone()
                    .map_err(|error| map_eval_failure(location, error))?,
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
    let mut cache: Vec<CachedResolution> = Vec::new();
    let mut mfp_isolation = IsolationCache::new();
    let mut wavesim_isolation = IsolationCache::new();
    let mut bundles = Vec::new();
    resolve_core(
        NumericalOwner::ModalFieldProjection,
        &decided,
        &decided.normalized.allocated.input.mfp_core.requirements,
        &mut mfp_isolation,
        &mut cache,
        &mut bundles,
    )?;
    resolve_core(
        NumericalOwner::Wavesim,
        &decided,
        &decided
            .normalized
            .allocated
            .input
            .wavesim_ddd_core
            .requirements,
        &mut wavesim_isolation,
        &mut cache,
        &mut bundles,
    )?;
    Ok(ResolvedExactNumericContext {
        decided,
        certified_descents: bundles,
        shared_resolution_count: cache.len(),
    })
}

fn resolve_core(
    owner: NumericalOwner,
    decided: &DecidedExactNumericContext,
    requirements: &[ExactExpressionRequirement],
    isolation: &mut IsolationCache,
    cache: &mut Vec<CachedResolution>,
    bundles: &mut Vec<CertifiedF64Bundle>,
) -> Result<(), NecoFailure> {
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
            let certified = match cache.iter().find(|entry| {
                entry.owner == owner
                    && entry.expr == *expression
                    && entry.bits == requirement.precision
            }) {
                Some(entry) => entry.value.try_clone().map_err(|error| {
                    map_resolve_failure(
                        location.with_expr(*expression),
                        ResolveError::Bigint(error),
                    )
                })?,
                None => {
                    let exact = decided
                        .normalized
                        .normal_form(requirement.consumer_id, *expression)
                        .ok_or(NecoFailure::UnknownExpression {
                            location: location.with_expr(*expression),
                        })?;
                    let resolved =
                        CertifiedF64::resolve(exact, *expression, requirement.precision, isolation)
                            .map_err(|error| {
                                map_float_failure(location.with_expr(*expression), error)
                            })?;
                    let cached = resolved.try_clone().map_err(|error| {
                        map_resolve_failure(
                            location.with_expr(*expression),
                            ResolveError::Bigint(error),
                        )
                    })?;
                    let required = cache.len().checked_add(1).ok_or_else(|| {
                        map_storage_failure(
                            location,
                            crate::StorageError::CapacityOverflow {
                                resource: StorageResource::ProductResolutionEntries,
                            },
                        )
                    })?;
                    reserve_entries(cache, required, StorageResource::ProductResolutionEntries)
                        .map_err(|error| map_storage_failure(location, error))?;
                    cache.push(CachedResolution {
                        owner,
                        expr: *expression,
                        bits: requirement.precision,
                        value: cached,
                    });
                    resolved
                }
            };
            values.push(CertifiedF64Expression {
                expr: *expression,
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
        bundles.push(CertifiedF64Bundle {
            consumer_id: requirement.consumer_id,
            values,
        });
    }
    Ok(())
}

pub fn assemble_exact_computation_product(
    resolved: ResolvedExactNumericContext,
) -> Result<ExactComputationProduct, NecoFailure> {
    let expected = resolved
        .decided
        .normalized
        .allocated
        .allocation
        .descent_consumers
        .len();
    if resolved.certified_descents.len() != expected || resolved.decided.decisions.len() != expected
    {
        return Err(NecoFailure::MissingAssignment {
            location: FailureLocation::operation(FailureOperation::Assemble),
        });
    }
    Ok(ExactComputationProduct { resolved })
}

fn normalize_core(
    owner: NumericalOwner,
    core: &impl CoreProduct,
    output: &mut Vec<NormalFormEntry>,
    evaluation_count: &mut usize,
) -> Result<(), NecoFailure> {
    let mut cache = EvaluationCache::new();
    for requirement in core.requirements() {
        for expression in requirement.expressions() {
            let location = FailureLocation::operation(FailureOperation::Normalize)
                .with_consumer(requirement.consumer_id())
                .with_expr(*expression);
            let before = cache.len();
            let value = evaluate_reachable(*expression, core.graph(), core.atoms(), &mut cache)
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
                value,
            });
        }
    }
    Ok(())
}

trait CoreProduct {
    fn graph(&self) -> &ExprGraph;
    fn atoms(&self) -> &AtomStore;
    fn requirements(&self) -> &[ExactExpressionRequirement];
}

impl CoreProduct for MfpCoreProduct {
    fn graph(&self) -> &ExprGraph {
        &self.graph
    }
    fn atoms(&self) -> &AtomStore {
        &self.atoms
    }
    fn requirements(&self) -> &[ExactExpressionRequirement] {
        &self.requirements
    }
}

impl CoreProduct for WavesimDddCoreProduct {
    fn graph(&self) -> &ExprGraph {
        &self.graph
    }
    fn atoms(&self) -> &AtomStore {
        &self.atoms
    }
    fn requirements(&self) -> &[ExactExpressionRequirement] {
        &self.requirements
    }
}

fn move_budgets(
    target: &mut Vec<NumericalErrorBudget>,
    source: &mut Vec<NumericalErrorBudget>,
) -> Result<(), NecoFailure> {
    let required = target.len().checked_add(source.len()).ok_or_else(|| {
        map_storage_failure(
            FailureLocation::operation(FailureOperation::Allocate),
            crate::StorageError::CapacityOverflow {
                resource: StorageResource::ProductEntries,
            },
        )
    })?;
    reserve_entries(target, required, StorageResource::ProductEntries).map_err(|error| {
        map_storage_failure(
            FailureLocation::operation(FailureOperation::Allocate),
            error,
        )
    })?;
    target.append(source);
    Ok(())
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

fn map_float_failure(location: FailureLocation, error: FloatError) -> NecoFailure {
    match error {
        FloatError::OutOfRange => NecoFailure::FloatOutOfRange { location },
        FloatError::Bigint(error) => map_resolve_failure(location, ResolveError::Bigint(error)),
        FloatError::Algnum(error) => map_resolve_failure(location, ResolveError::Algnum(error)),
        FloatError::Storage(error) => map_storage_failure(location, error),
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

fn all_exact_inputs() -> Vec<ExactInput> {
    alloc::vec![
        ExactInput::GeometryIdentity,
        ExactInput::GeometryDimension,
        ExactInput::SourceNode,
        ExactInput::ReceiverNode,
        ExactInput::FrequencyBandEndpoints,
        ExactInput::ModeLimit,
        ExactInput::SamplingCount,
        ExactInput::SamplingRate,
        ExactInput::ModeIndexOrdering,
        ExactInput::ModeRowCardinality,
        ExactInput::ModeRowWidth,
        ExactInput::DampingDefinition,
        ExactInput::SystemIdentity,
        ExactInput::SubsystemIdentity,
        ExactInput::StateShape,
        ExactInput::StateExtents,
        ExactInput::InitialStateIndex,
        ExactInput::TimeDomainEndpoints,
        ExactInput::CalibrationInterval,
        ExactInput::HeldOutInterval,
        ExactInput::ConditionIdentity,
        ExactInput::CouplingTopology,
        ExactInput::CouplingSelector,
        ExactInput::ComparatorDirection,
        ExactInput::AcceptanceDomainBound,
    ]
}

fn all_exact_decisions() -> Vec<ExactDecisionAssignment> {
    alloc::vec![
        ExactDecisionAssignment::ProvenanceNonempty,
        ExactDecisionAssignment::SourceReceiverIdentityValidity,
        ExactDecisionAssignment::ModeSetNonempty,
        ExactDecisionAssignment::ModeIdentityEqualityOrdering,
        ExactDecisionAssignment::ModeShapeCardinalityAxisEquality,
        ExactDecisionAssignment::SamplingDomainValidity,
        ExactDecisionAssignment::ZeroDivisionGuard,
        ExactDecisionAssignment::Rt60RoundTripEquality,
        ExactDecisionAssignment::MfpBranchIdentity,
        ExactDecisionAssignment::StateShapeEquality,
        ExactDecisionAssignment::CalibrationHeldOutDisjointness,
        ExactDecisionAssignment::ConditionSetCompleteness,
        ExactDecisionAssignment::SubsystemIdentityEquality,
        ExactDecisionAssignment::FiniteNonNegative,
        ExactDecisionAssignment::EnergyBalance,
        ExactDecisionAssignment::SingularityZeroDenominator,
        ExactDecisionAssignment::PredictionIndependenceEquality,
        ExactDecisionAssignment::R2LowerBound,
        ExactDecisionAssignment::AssemblyFailureSetEmptiness,
    ]
}

fn all_numerical_operations() -> Vec<NumericalOperation> {
    alloc::vec![
        NumericalOperation::ModeFrequencyEvaluation,
        NumericalOperation::ModeShapeEvaluation,
        NumericalOperation::DampingRateCalculation,
        NumericalOperation::ModeContributionCalculation,
        NumericalOperation::ModeSum,
        NumericalOperation::ReceivedSeriesSamplingAccumulation,
        NumericalOperation::TranscendentalEvaluationFem,
        NumericalOperation::ModalRhsEvaluation,
        NumericalOperation::InitialStateNumericalConstruction,
        NumericalOperation::OdeIntegration,
        NumericalOperation::EnergyObservation,
        NumericalOperation::DddRegressionEstimation,
        NumericalOperation::SeaRegressionEstimation,
        NumericalOperation::HeldOutPredictionComparison,
    ]
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
    use crate::storage::{with_injected_failure, InjectedFailure};
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
        let mfp = MfpCoreProduct::new(graph, atoms, vec![requirement(0, expr)], vec![]).unwrap();
        let (wavesim_graph, wavesim_atoms, _) = domain(Some(ExactValue::Monomial(Monomial::one())));
        let wavesim =
            WavesimDddCoreProduct::new(wavesim_graph, wavesim_atoms, vec![], vec![]).unwrap();
        ExactAllocationInput::new(
            mfp,
            wavesim,
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
                payload: StorageFailurePayload::Storage(StorageError::AllocationFailure {
                    resource: StorageResource::ProductEntries,
                    requested_elements: 1,
                }),
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
        malformed.input.mfp_core.requirements[0].expressions[0] = ExprId::new(99);
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
            Err(NecoFailure::StorageFailure {
                location,
                payload: StorageFailurePayload::Storage(StorageError::AllocationFailure {
                    resource: StorageResource::EvaluationEntries,
                    ..
                }),
            }) if location.operation_kind() == FailureOperation::Normalize
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
            Err(NecoFailure::StorageFailure {
                location,
                payload: StorageFailurePayload::Storage(StorageError::AllocationFailure {
                    resource: StorageResource::ProductEntries,
                    ..
                }),
            }) if location.operation_kind() == FailureOperation::Decide
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
            Err(NecoFailure::StorageFailure {
                location,
                payload: StorageFailurePayload::Storage(StorageError::AllocationFailure {
                    resource: StorageResource::ProductResolutionEntries,
                    ..
                }),
            }) if location.operation_kind() == FailureOperation::Resolve
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
}
