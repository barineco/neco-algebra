use neco_bigint::{BigInt, BigUint, RawRational, Sign};
use neco_expr::{
    AbsoluteBits, Assignments, AtomId, AtomStore, ConsumerId, ExactValue, ExprId, InsertError,
    PrecisionRequirements, StorageResource,
};
use neco_formsum::{RawFormSum, RawTerm};
use neco_monomial::RawMonomial;

fn form(value: i32) -> neco_formsum::FormSum {
    RawFormSum::new(vec![RawTerm::new(
        RawRational::new(
            BigInt::from_sign_magnitude(
                if value < 0 {
                    Sign::Negative
                } else {
                    Sign::Positive
                },
                BigUint::try_from(value.unsigned_abs()).unwrap(),
            ),
            BigUint::one().unwrap(),
        ),
        RawMonomial::positive(Vec::new()),
    )])
    .normalize()
    .unwrap()
}

#[test]
fn atom_map_is_sorted_and_duplicate_preserves_the_value() {
    let mut left = AtomStore::new();
    left.insert(AtomId::new(7), ExactValue::FormSum(form(7)))
        .unwrap();
    left.insert(AtomId::new(2), ExactValue::FormSum(form(2)))
        .unwrap();
    let before = left.try_clone().unwrap();
    assert_eq!(
        left.insert(AtomId::new(2), ExactValue::FormSum(form(99))),
        Err(InsertError::DuplicateId {
            resource: StorageResource::AtomEntries,
            id: 2,
        })
    );
    assert_eq!(left, before);

    let mut right = AtomStore::new();
    right
        .insert(AtomId::new(2), ExactValue::FormSum(form(2)))
        .unwrap();
    right
        .insert(AtomId::new(7), ExactValue::FormSum(form(7)))
        .unwrap();
    assert_eq!(left, right);
    left.set(AtomId::new(2), ExactValue::FormSum(form(3)))
        .unwrap();
    assert_eq!(
        left.get(AtomId::new(2)),
        Some(&ExactValue::FormSum(form(3)))
    );
    assert_eq!(left.len(), 2);
}

#[test]
fn precision_map_is_sorted_and_duplicate_preserves_the_value() {
    let mut left = PrecisionRequirements::new();
    left.insert(ConsumerId::new(9), AbsoluteBits::new(90))
        .unwrap();
    left.insert(ConsumerId::new(1), AbsoluteBits::new(10))
        .unwrap();
    let before = left.try_clone().unwrap();
    assert_eq!(
        left.insert(ConsumerId::new(1), AbsoluteBits::new(99)),
        Err(InsertError::DuplicateId {
            resource: StorageResource::PrecisionEntries,
            id: 1,
        })
    );
    assert_eq!(left, before);

    let mut right = PrecisionRequirements::new();
    right
        .insert(ConsumerId::new(1), AbsoluteBits::new(10))
        .unwrap();
    right
        .insert(ConsumerId::new(9), AbsoluteBits::new(90))
        .unwrap();
    assert_eq!(left, right);
    left.set(ConsumerId::new(1), AbsoluteBits::new(11)).unwrap();
    assert_eq!(left.get(ConsumerId::new(1)), Some(AbsoluteBits::new(11)));
}

#[test]
fn assignment_map_is_sorted_and_duplicate_preserves_the_value() {
    let mut left = Assignments::new();
    left.insert(ConsumerId::new(8), ExprId::new(80)).unwrap();
    left.insert(ConsumerId::new(3), ExprId::new(30)).unwrap();
    let before = left.try_clone().unwrap();
    assert_eq!(
        left.insert(ConsumerId::new(3), ExprId::new(31)),
        Err(InsertError::DuplicateId {
            resource: StorageResource::AssignmentEntries,
            id: 3,
        })
    );
    assert_eq!(left, before);

    let mut right = Assignments::new();
    right.insert(ConsumerId::new(3), ExprId::new(30)).unwrap();
    right.insert(ConsumerId::new(8), ExprId::new(80)).unwrap();
    assert_eq!(left, right);
    left.set(ConsumerId::new(3), ExprId::new(32)).unwrap();
    assert_eq!(left.get(ConsumerId::new(3)), Some(ExprId::new(32)));
}

#[test]
fn storage_resource_inventory_is_complete_and_ordered() {
    let resources = [
        StorageResource::GraphNodes,
        StorageResource::AtomEntries,
        StorageResource::PrecisionEntries,
        StorageResource::AssignmentEntries,
        StorageResource::EvaluationEntries,
        StorageResource::IsolationEntries,
        StorageResource::ResolvedEntries,
    ];
    assert!(resources.windows(2).all(|pair| pair[0] < pair[1]));
}
