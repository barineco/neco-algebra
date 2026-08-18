use crate::evaluate::{evaluate_reachable, EvaluationRunError};
use crate::float::FloatError;
use crate::{
    Assignments, AtomStore, CertifiedF64, ConsumerId, EvaluationCache, ExprGraph, ExprId,
    IsolationCache, PrecisionRequirements, ResolveError, ResolvedValues, StorageError,
};

#[derive(Debug, Eq, PartialEq)]
pub struct Resolver;

#[allow(clippy::new_without_default)]
impl Resolver {
    pub const fn new() -> Self {
        Self
    }

    pub fn resolve_all(
        &self,
        graph: &ExprGraph,
        atoms: &AtomStore,
        requirements: &PrecisionRequirements,
        assignments: &Assignments,
    ) -> Result<(EvaluationCache, IsolationCache, ResolvedValues), ResolveError> {
        let mut evaluation = EvaluationCache::new();
        let mut isolation = IsolationCache::new();
        let mut resolved = ResolvedValues::new();

        for &(consumer, bits) in requirements.entries() {
            let result = match assignments.get(consumer) {
                None => Err(ResolveError::MissingAssignment { consumer }),
                Some(expr) if graph.get(expr).is_none() => {
                    Err(ResolveError::UnknownExprId { consumer, expr })
                }
                Some(expr) => resolve_consumer(
                    consumer,
                    expr,
                    bits,
                    graph,
                    atoms,
                    &mut evaluation,
                    &mut isolation,
                )
                .map_err(ResolveError::Storage)?,
            };
            insert_consumer_result(&mut resolved, consumer, result)?;
        }
        Ok((evaluation, isolation, resolved))
    }
}

fn insert_consumer_result(
    resolved: &mut ResolvedValues,
    consumer: ConsumerId,
    result: Result<CertifiedF64, ResolveError>,
) -> Result<(), ResolveError> {
    resolved
        .insert(consumer, result)
        .map_err(ResolveError::Storage)
}

#[allow(clippy::too_many_arguments)]
fn resolve_consumer(
    consumer: ConsumerId,
    expr: ExprId,
    bits: crate::AbsoluteBits,
    graph: &ExprGraph,
    atoms: &AtomStore,
    evaluation: &mut EvaluationCache,
    isolation: &mut IsolationCache,
) -> Result<Result<CertifiedF64, ResolveError>, StorageError> {
    let value = match evaluate_reachable(expr, graph, atoms, evaluation) {
        Ok(value) => value,
        Err(EvaluationRunError::UnknownExpr(expr)) => {
            return Ok(Err(ResolveError::UnknownExprId { consumer, expr }));
        }
        Err(EvaluationRunError::UnknownAtom { expr, atom }) => {
            return Ok(Err(ResolveError::UnknownAtomId { expr, atom }));
        }
        Err(EvaluationRunError::Evaluation(error)) => {
            return Ok(Err(local_evaluation_error(error)));
        }
        Err(EvaluationRunError::Storage(error)) => return Err(error),
    };
    match CertifiedF64::resolve(&value, expr, bits, isolation) {
        Ok(value) => Ok(Ok(value)),
        Err(FloatError::OutOfRange) => Ok(Err(ResolveError::FloatOutOfRange { consumer, expr })),
        Err(FloatError::Bigint(error)) => Ok(Err(ResolveError::Bigint(error))),
        Err(FloatError::Algnum(error)) => Ok(Err(ResolveError::Algnum(error))),
        Err(FloatError::Storage(error)) => Err(error),
    }
}

fn local_evaluation_error(error: crate::EvalError) -> ResolveError {
    ResolveError::Evaluation(error)
}
