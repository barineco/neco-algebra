use alloc::vec::Vec;

use crate::error::try_clone_eval_error;
use crate::storage::{required_elements, reserve_entries, StorageResource};
use crate::value::{add_exact, div_exact, mul_exact, neg_exact, pow_exact, sub_exact};
use crate::{
    AtomId, AtomStore, EvalError, EvaluationCache, ExactValue, ExprGraph, ExprId, ExprNode,
    StorageError,
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum EvaluationRunError {
    UnknownExpr(ExprId),
    UnknownAtom { expr: ExprId, atom: AtomId },
    Evaluation(EvalError),
    Storage(StorageError),
}

enum NodeFailure {
    UnknownAtom { expr: ExprId, atom: AtomId },
    Evaluation(EvalError),
    Unavailable(ExprId),
}

struct UnknownEntry {
    node: ExprId,
    expr: ExprId,
    atom: AtomId,
}

pub(crate) fn evaluate_reachable(
    root: ExprId,
    graph: &ExprGraph,
    atoms: &AtomStore,
    cache: &mut EvaluationCache,
) -> Result<ExactValue, EvaluationRunError> {
    if graph.get(root).is_none() {
        return Err(EvaluationRunError::UnknownExpr(root));
    }
    if let Some(result) = clone_cached_entry(cache, root) {
        return result.map_err(EvaluationRunError::Evaluation);
    }

    let reachable = collect_reachable(root, graph).map_err(EvaluationRunError::Storage)?;
    let mut unknowns = Vec::new();
    for id in reachable {
        if cache.get(id).is_some() {
            continue;
        }
        let node = match graph.get(id) {
            Some(node) => node,
            None => return Err(EvaluationRunError::UnknownExpr(id)),
        };
        match evaluate_node(node, id, atoms, cache, &unknowns) {
            Ok(value) => cache
                .insert(id, Ok(value))
                .map_err(EvaluationRunError::Storage)?,
            Err(NodeFailure::Evaluation(error)) => cache
                .insert(id, Err(error))
                .map_err(EvaluationRunError::Storage)?,
            Err(NodeFailure::UnknownAtom { expr, atom }) => {
                insert_unknown(
                    &mut unknowns,
                    UnknownEntry {
                        node: id,
                        expr,
                        atom,
                    },
                )
                .map_err(EvaluationRunError::Storage)?;
            }
            Err(NodeFailure::Unavailable(expr)) => {
                return Err(EvaluationRunError::UnknownExpr(expr));
            }
        }
    }

    match operand(root, cache, &unknowns) {
        Ok(value) => Ok(value),
        Err(NodeFailure::UnknownAtom { expr, atom }) => {
            Err(EvaluationRunError::UnknownAtom { expr, atom })
        }
        Err(NodeFailure::Evaluation(error)) => Err(EvaluationRunError::Evaluation(error)),
        Err(NodeFailure::Unavailable(expr)) => Err(EvaluationRunError::UnknownExpr(expr)),
    }
}

fn collect_reachable(root: ExprId, graph: &ExprGraph) -> Result<Vec<ExprId>, StorageError> {
    let mut pending = Vec::new();
    push_evaluation_entry(&mut pending, root)?;
    let mut reachable = Vec::new();
    while let Some(id) = pending.pop() {
        let index = match reachable.binary_search(&id) {
            Ok(_) => continue,
            Err(index) => index,
        };
        let required = required_elements(reachable.len(), StorageResource::EvaluationEntries)?;
        reserve_entries(&mut reachable, required, StorageResource::EvaluationEntries)?;
        reachable.insert(index, id);
        let node = match graph.get(id) {
            Some(node) => node,
            None => continue,
        };
        match node {
            ExprNode::Atom(_) => {}
            ExprNode::Neg(child) => push_evaluation_entry(&mut pending, *child)?,
            ExprNode::Add(left, right)
            | ExprNode::Sub(left, right)
            | ExprNode::Mul(left, right)
            | ExprNode::Div(left, right) => {
                push_evaluation_entry(&mut pending, *left)?;
                push_evaluation_entry(&mut pending, *right)?;
            }
            ExprNode::Pow { base, .. } => push_evaluation_entry(&mut pending, *base)?,
        }
    }
    Ok(reachable)
}

fn push_evaluation_entry<T>(entries: &mut Vec<T>, value: T) -> Result<(), StorageError> {
    let required = required_elements(entries.len(), StorageResource::EvaluationEntries)?;
    reserve_entries(entries, required, StorageResource::EvaluationEntries)?;
    entries.push(value);
    Ok(())
}

fn evaluate_node(
    node: &ExprNode,
    expr: ExprId,
    atoms: &AtomStore,
    cache: &mut EvaluationCache,
    unknowns: &[UnknownEntry],
) -> Result<ExactValue, NodeFailure> {
    match node {
        ExprNode::Atom(atom) => match atoms.get(*atom) {
            Some(value) => value.try_clone().map_err(NodeFailure::Evaluation),
            None => Err(NodeFailure::UnknownAtom { expr, atom: *atom }),
        },
        ExprNode::Neg(child) => {
            let value = operand(*child, cache, unknowns)?;
            neg_exact(&value).map_err(NodeFailure::Evaluation)
        }
        ExprNode::Add(left, right) => evaluate_binary(*left, *right, cache, unknowns, add_exact),
        ExprNode::Sub(left, right) => evaluate_binary(*left, *right, cache, unknowns, sub_exact),
        ExprNode::Mul(left, right) => evaluate_binary(*left, *right, cache, unknowns, mul_exact),
        ExprNode::Div(left, right) => evaluate_binary(*left, *right, cache, unknowns, div_exact),
        ExprNode::Pow { base, exponent } => {
            let value = operand(*base, cache, unknowns)?;
            pow_exact(&value, exponent).map_err(NodeFailure::Evaluation)
        }
    }
}

fn evaluate_binary(
    left: ExprId,
    right: ExprId,
    cache: &mut EvaluationCache,
    unknowns: &[UnknownEntry],
    operation: fn(&ExactValue, &ExactValue) -> Result<ExactValue, EvalError>,
) -> Result<ExactValue, NodeFailure> {
    let left_result = operand(left, cache, unknowns);
    let right_result = operand(right, cache, unknowns);
    match (left_result, right_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(left), Ok(right)) => operation(&left, &right).map_err(NodeFailure::Evaluation),
    }
}

fn operand(
    id: ExprId,
    cache: &mut EvaluationCache,
    unknowns: &[UnknownEntry],
) -> Result<ExactValue, NodeFailure> {
    if let Some(result) = clone_cached_entry(cache, id) {
        return result.map_err(NodeFailure::Evaluation);
    }
    match unknowns.binary_search_by_key(&id, |entry| entry.node) {
        Ok(index) => Err(NodeFailure::UnknownAtom {
            expr: unknowns[index].expr,
            atom: unknowns[index].atom,
        }),
        Err(_) => Err(NodeFailure::Unavailable(id)),
    }
}

fn clone_cached_entry(
    cache: &mut EvaluationCache,
    id: ExprId,
) -> Option<Result<ExactValue, EvalError>> {
    cache.get_mut(id).map(|result| clone_cached_result(result))
}

fn clone_cached_result(result: &Result<ExactValue, EvalError>) -> Result<ExactValue, EvalError> {
    match result {
        Ok(value) => value.try_clone(),
        Err(error) => Err(try_clone_eval_error(error)?),
    }
}

fn insert_unknown(
    entries: &mut Vec<UnknownEntry>,
    entry: UnknownEntry,
) -> Result<(), StorageError> {
    let index = match entries.binary_search_by_key(&entry.node, |candidate| candidate.node) {
        Ok(index) | Err(index) => index,
    };
    let required = required_elements(entries.len(), StorageResource::EvaluationEntries)?;
    reserve_entries(entries, required, StorageResource::EvaluationEntries)?;
    entries.insert(index, entry);
    Ok(())
}

#[cfg(test)]
mod tests {
    use neco_bigint::{BigInt, ReducedRational};
    use neco_monomial::Monomial;

    use super::{evaluate_reachable, EvaluationRunError};
    use crate::storage::{with_injected_failure, InjectedFailure, StorageResource};
    use crate::value::{with_clone_failure, CloneContact};
    use crate::{AtomId, AtomStore, EvalError, EvaluationCache, ExactValue, ExprGraph, ExprNode};

    fn integer(value: i32) -> ReducedRational {
        ReducedRational::from_bigint(BigInt::try_from(value).unwrap()).unwrap()
    }

    #[test]
    fn binary_evaluation_prefers_left_failure_and_caches_both_children() {
        let mut graph = ExprGraph::new();
        let mut atoms = AtomStore::new();
        atoms
            .insert(AtomId::new(0), ExactValue::Monomial(Monomial::zero()))
            .unwrap();
        let zero = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let left = graph
            .push(ExprNode::Pow {
                base: zero,
                exponent: integer(0),
            })
            .unwrap();
        let right = graph
            .push(ExprNode::Pow {
                base: zero,
                exponent: integer(-1),
            })
            .unwrap();
        let parent = graph.push(ExprNode::Add(left, right)).unwrap();
        let mut cache = EvaluationCache::new();
        assert_eq!(
            evaluate_reachable(parent, &graph, &atoms, &mut cache),
            Err(EvaluationRunError::Evaluation(
                EvalError::UndefinedZeroPower
            ))
        );
        assert_eq!(cache.get(left), Some(&Err(EvalError::UndefinedZeroPower)));
        assert_eq!(cache.get(right), Some(&Err(EvalError::ZeroToNegativePower)));
        assert_eq!(cache.get(parent), Some(&Err(EvalError::UndefinedZeroPower)));
    }

    #[test]
    fn cached_root_reuse_performs_no_new_evaluation_storage() {
        let mut graph = ExprGraph::new();
        let mut atoms = AtomStore::new();
        atoms
            .insert(AtomId::new(0), ExactValue::Monomial(Monomial::one()))
            .unwrap();
        let root = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let mut cache = EvaluationCache::new();
        assert!(evaluate_reachable(root, &graph, &atoms, &mut cache).is_ok());
        let reused = with_injected_failure(
            StorageResource::EvaluationEntries,
            InjectedFailure::Allocation,
            || evaluate_reachable(root, &graph, &atoms, &mut cache),
        );
        assert!(reused.is_ok());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn cached_value_clone_failure_preserves_the_existing_entry() {
        let mut graph = ExprGraph::new();
        let mut atoms = AtomStore::new();
        atoms
            .insert(AtomId::new(0), ExactValue::Monomial(Monomial::one()))
            .unwrap();
        let root = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let mut cache = EvaluationCache::new();
        assert!(evaluate_reachable(root, &graph, &atoms, &mut cache).is_ok());
        assert_eq!(
            with_clone_failure(CloneContact::Monomial, || evaluate_reachable(
                root, &graph, &atoms, &mut cache
            )),
            Err(EvaluationRunError::Evaluation(EvalError::Monomial(
                neco_monomial::MonomialErrorKind::AllocationFailure {
                    requested_elements: 19,
                }
            )))
        );
        assert!(cache.get(root).unwrap().is_ok());
        assert!(evaluate_reachable(root, &graph, &atoms, &mut cache).is_ok());
    }
}
