use neco_bigint::{BigInt, ReducedRational};
use neco_expr::{AtomId, ExprGraph, ExprId, ExprNode, GraphError};

fn integer(value: i32) -> ReducedRational {
    ReducedRational::from_bigint(BigInt::try_from(value).unwrap()).unwrap()
}

#[test]
fn graph_accepts_every_node_variant_in_addition_order() {
    let mut graph = ExprGraph::new();
    let atom = graph.push(ExprNode::Atom(AtomId::new(9))).unwrap();
    let neg = graph.push(ExprNode::Neg(atom)).unwrap();
    let add = graph.push(ExprNode::Add(atom, neg)).unwrap();
    let sub = graph.push(ExprNode::Sub(add, atom)).unwrap();
    let mul = graph.push(ExprNode::Mul(sub, neg)).unwrap();
    let div = graph.push(ExprNode::Div(mul, atom)).unwrap();
    let pow = graph
        .push(ExprNode::Pow {
            base: div,
            exponent: integer(2),
        })
        .unwrap();

    assert_eq!(
        [atom, neg, add, sub, mul, div, pow],
        [
            ExprId::new(0),
            ExprId::new(1),
            ExprId::new(2),
            ExprId::new(3),
            ExprId::new(4),
            ExprId::new(5),
            ExprId::new(6),
        ]
    );
    assert_eq!(graph.len(), 7);
    assert_eq!(graph.get(ExprId::new(7)), None);
    assert_eq!(graph.try_clone().unwrap(), graph);
}

#[test]
fn every_child_position_rejects_self_and_future_ids_without_writing() {
    let mut graph = ExprGraph::new();
    graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();

    let invalid = [
        ExprNode::Neg(ExprId::new(1)),
        ExprNode::Add(ExprId::new(1), ExprId::new(0)),
        ExprNode::Add(ExprId::new(0), ExprId::new(2)),
        ExprNode::Sub(ExprId::new(3), ExprId::new(0)),
        ExprNode::Mul(ExprId::new(0), ExprId::new(4)),
        ExprNode::Div(ExprId::new(5), ExprId::new(0)),
        ExprNode::Pow {
            base: ExprId::new(6),
            exponent: integer(1),
        },
    ];
    for node in invalid {
        let before = graph.try_clone().unwrap();
        let child = match &node {
            ExprNode::Neg(child) => *child,
            ExprNode::Add(left, right)
            | ExprNode::Sub(left, right)
            | ExprNode::Mul(left, right)
            | ExprNode::Div(left, right) => {
                if *left >= ExprId::new(1) {
                    *left
                } else {
                    *right
                }
            }
            ExprNode::Pow { base, .. } => *base,
            ExprNode::Atom(_) => unreachable!(),
        };
        assert_eq!(
            graph.push(node),
            Err(GraphError::InvalidChildId {
                child,
                next: ExprId::new(1),
            })
        );
        assert_eq!(graph, before);
    }
}
