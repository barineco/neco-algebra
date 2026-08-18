#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;
#[cfg(test)]
extern crate std;

mod error;
mod evaluate;
mod float;
mod graph;
mod id;
mod product;
mod resolve;
mod storage;
mod value;

pub use error::{
    EvalError, FailureLocation, FailureOperation, GraphError, InsertError, NecoFailure,
    ResolveError, StorageError, StorageFailurePayload, UnsupportedFailurePayload,
};
pub use float::{
    project_exact_value_f64, CertifiedF64, CertifiedScalarProjection, ProjectionPolicy,
    ScalarProjectionError,
};
pub use graph::ExprGraph;
pub use id::{AbsoluteBits, AtomId, ConsumerId, ExprId};
pub use product::{
    allocate_exact_numeric, assemble_exact_computation_product, decide_exact_properties,
    normalize_exact_expressions, read_exact_numeric_inputs, resolve_certified_f64,
    AllocatedExactNumericContext, CertifiedF64Bundle, CertifiedF64Expression,
    DecidedExactNumericContext, DirectInspection, ExactAllocationInput, ExactComputationProduct,
    ExactDecisionAssignment, ExactDecisionKind, ExactDecisionRequest, ExactDecisionValue,
    ExactDecisionWitness, ExactExpressionRequirement, ExactInput, ExactNumericAllocation,
    ExactOperation, MfpCoreProduct, NecoImplementationSource, NecoObservedCapability,
    NormalizedExactNumericContext, NumericalBudgetComponent, NumericalErrorBudget,
    NumericalOperation, NumericalOwner, ResolvedExactNumericContext, WavesimDddCoreProduct,
};
pub use resolve::Resolver;
pub use storage::{
    Assignments, AtomStore, EvaluationCache, IsolationCache, PrecisionRequirements, ResolvedValues,
    StorageResource,
};
pub use value::{ExactLayer, ExactValue, ExprNode};
