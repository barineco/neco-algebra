use alloc::vec::Vec;

use crate::storage::{reserve_entries, StorageResource};
use crate::{ExprId, ExprNode, GraphError};

#[derive(Debug, Eq, PartialEq)]
pub struct ExprGraph {
    nodes: Vec<ExprNode>,
}

#[allow(clippy::new_without_default)]
impl ExprGraph {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn get(&self, id: ExprId) -> Option<&ExprNode> {
        usize::try_from(id.get())
            .ok()
            .and_then(|index| self.nodes.get(index))
    }

    pub fn push(&mut self, node: ExprNode) -> Result<ExprId, GraphError> {
        let next = next_expr_id(self.nodes.len())?;
        validate_children(&node, next)?;
        let required = self
            .nodes
            .len()
            .checked_add(1)
            .ok_or(GraphError::IdExhausted)?;
        reserve_entries(&mut self.nodes, required, StorageResource::GraphNodes)
            .map_err(GraphError::Storage)?;
        self.nodes.push(node);
        Ok(next)
    }

    pub fn try_clone(&self) -> Result<Self, GraphError> {
        let mut nodes = Vec::new();
        reserve_entries(&mut nodes, self.nodes.len(), StorageResource::GraphNodes)
            .map_err(GraphError::Storage)?;
        for node in &self.nodes {
            nodes.push(node.try_clone().map_err(graph_node_error)?);
        }
        Ok(Self { nodes })
    }
}

fn graph_node_error(error: crate::EvalError) -> GraphError {
    GraphError::Node(graph_node_payload(error))
}

fn graph_node_payload(error: crate::EvalError) -> crate::EvalError {
    error
}

fn next_expr_id(length: usize) -> Result<ExprId, GraphError> {
    u32::try_from(length)
        .map(ExprId::new)
        .map_err(|_| GraphError::IdExhausted)
}

fn validate_child(child: ExprId, next: ExprId) -> Result<(), GraphError> {
    if child < next {
        Ok(())
    } else {
        Err(GraphError::InvalidChildId { child, next })
    }
}

fn validate_children(node: &ExprNode, next: ExprId) -> Result<(), GraphError> {
    match node {
        ExprNode::Atom(_) => Ok(()),
        ExprNode::Neg(child) => validate_child(*child, next),
        ExprNode::Add(left, right)
        | ExprNode::Sub(left, right)
        | ExprNode::Mul(left, right)
        | ExprNode::Div(left, right) => {
            validate_child(*left, next)?;
            validate_child(*right, next)
        }
        ExprNode::Pow { base, .. } => validate_child(*base, next),
    }
}

#[cfg(test)]
mod tests {
    use neco_bigint::{BigInt, BigintError, ReducedRational};

    use super::next_expr_id;
    use crate::value::{with_clone_failure, CloneContact};
    use crate::{AtomId, EvalError, ExprGraph, ExprId, ExprNode, GraphError};

    #[test]
    fn next_id_rejects_values_above_u32() {
        if usize::BITS > u32::BITS {
            assert_eq!(
                next_expr_id((u32::MAX as usize) + 1),
                Err(GraphError::IdExhausted)
            );
        }
    }

    #[test]
    fn graph_accepts_only_existing_children() {
        let mut graph = ExprGraph::new();
        let atom = graph.push(ExprNode::Atom(AtomId::new(4))).unwrap();
        assert_eq!(atom, ExprId::new(0));

        let add = graph
            .push(ExprNode::Add(ExprId::new(0), ExprId::new(0)))
            .unwrap();
        assert_eq!(add, ExprId::new(1));
        assert_eq!(graph.len(), 2);

        assert_eq!(
            graph.push(ExprNode::Neg(ExprId::new(2))),
            Err(GraphError::InvalidChildId {
                child: ExprId::new(2),
                next: ExprId::new(2),
            })
        );
        assert_eq!(graph.len(), 2);

        assert_eq!(
            graph.push(ExprNode::Mul(ExprId::new(0), ExprId::new(3))),
            Err(GraphError::InvalidChildId {
                child: ExprId::new(3),
                next: ExprId::new(2),
            })
        );
        assert_eq!(graph.len(), 2);
    }

    #[test]
    fn graph_clone_preserves_node_order() {
        let mut graph = ExprGraph::new();
        graph.push(ExprNode::Atom(AtomId::new(7))).unwrap();
        graph.push(ExprNode::Neg(ExprId::new(0))).unwrap();

        assert_eq!(graph.try_clone().unwrap(), graph);
    }

    #[test]
    fn exponent_clone_failure_keeps_the_bigint_payload() {
        let node = ExprNode::Pow {
            base: ExprId::new(0),
            exponent: ReducedRational::from_bigint(BigInt::one().unwrap()).unwrap(),
        };
        assert_eq!(
            with_clone_failure(CloneContact::Exponent, || node.try_clone()),
            Err(EvalError::Bigint(BigintError::AllocationFailure {
                requested_limbs: 17,
            }))
        );
    }

    #[test]
    fn graph_clone_wraps_node_failure_and_preserves_its_payload() {
        let mut graph = ExprGraph::new();
        let atom = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        graph
            .push(ExprNode::Pow {
                base: atom,
                exponent: ReducedRational::from_bigint(BigInt::one().unwrap()).unwrap(),
            })
            .unwrap();
        assert_eq!(
            with_clone_failure(CloneContact::Exponent, || graph.try_clone()),
            Err(GraphError::Node(EvalError::Bigint(
                BigintError::AllocationFailure {
                    requested_limbs: 17,
                }
            )))
        );
        assert_eq!(graph.len(), 2);
    }
}
