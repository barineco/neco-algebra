use alloc::vec::Vec;
use core::mem::size_of;

use crate::{
    AbsoluteBits, AtomId, CertifiedF64, ConsumerId, EvalError, ExactValue, ExprId, InsertError,
    ResolveError, StorageError,
};
use neco_algnum::IsolatingInterval;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StorageResource {
    GraphNodes,
    AtomEntries,
    PrecisionEntries,
    AssignmentEntries,
    EvaluationEntries,
    IsolationEntries,
    ResolvedEntries,
    ProductEntries,
    ProductResolutionEntries,
}

#[derive(Debug, Eq, PartialEq)]
pub struct AtomStore {
    entries: Vec<(AtomId, ExactValue)>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct PrecisionRequirements {
    entries: Vec<(ConsumerId, AbsoluteBits)>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Assignments {
    entries: Vec<(ConsumerId, ExprId)>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct EvaluationCache {
    entries: Vec<(ExprId, Result<ExactValue, EvalError>)>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct IsolationCache {
    entries: Vec<((ExprId, AbsoluteBits), IsolatingInterval)>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ResolvedValues {
    entries: Vec<(ConsumerId, Result<CertifiedF64, ResolveError>)>,
}

#[allow(clippy::len_without_is_empty, clippy::new_without_default)]
impl AtomStore {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, id: AtomId) -> Option<&ExactValue> {
        get_entry(&self.entries, id)
    }

    pub fn insert(&mut self, id: AtomId, value: ExactValue) -> Result<(), InsertError> {
        insert_entry(
            &mut self.entries,
            id,
            value,
            StorageResource::AtomEntries,
            id.get(),
        )
    }

    pub fn set(&mut self, id: AtomId, value: ExactValue) -> Result<(), InsertError> {
        set_entry(
            &mut self.entries,
            id,
            value,
            StorageResource::AtomEntries,
            id.get(),
        )
    }

    pub fn try_clone(&self) -> Result<Self, InsertError> {
        let mut entries = Vec::new();
        reserve_entries(
            &mut entries,
            self.entries.len(),
            StorageResource::AtomEntries,
        )?;
        for (id, value) in &self.entries {
            entries.push((*id, value.try_clone().map_err(atom_value_error)?));
        }
        Ok(Self { entries })
    }
}

fn atom_value_error(error: EvalError) -> InsertError {
    InsertError::Value(atom_value_payload(error))
}

fn atom_value_payload(error: EvalError) -> EvalError {
    error
}

#[allow(clippy::len_without_is_empty, clippy::new_without_default)]
impl PrecisionRequirements {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, id: ConsumerId) -> Option<AbsoluteBits> {
        get_entry(&self.entries, id).copied()
    }

    pub fn insert(&mut self, id: ConsumerId, bits: AbsoluteBits) -> Result<(), InsertError> {
        insert_entry(
            &mut self.entries,
            id,
            bits,
            StorageResource::PrecisionEntries,
            id.get(),
        )
    }

    pub fn set(&mut self, id: ConsumerId, bits: AbsoluteBits) -> Result<(), InsertError> {
        set_entry(
            &mut self.entries,
            id,
            bits,
            StorageResource::PrecisionEntries,
            id.get(),
        )
    }

    pub fn try_clone(&self) -> Result<Self, InsertError> {
        Ok(Self {
            entries: try_clone_copy_entries(&self.entries, StorageResource::PrecisionEntries)?,
        })
    }

    pub(crate) fn entries(&self) -> &[(ConsumerId, AbsoluteBits)] {
        &self.entries
    }
}

#[allow(clippy::len_without_is_empty, clippy::new_without_default)]
impl Assignments {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, id: ConsumerId) -> Option<ExprId> {
        get_entry(self.entries(), id).copied()
    }

    pub fn insert(&mut self, id: ConsumerId, expr: ExprId) -> Result<(), InsertError> {
        insert_entry(
            &mut self.entries,
            id,
            expr,
            StorageResource::AssignmentEntries,
            id.get(),
        )
    }

    pub fn set(&mut self, id: ConsumerId, expr: ExprId) -> Result<(), InsertError> {
        set_entry(
            &mut self.entries,
            id,
            expr,
            StorageResource::AssignmentEntries,
            id.get(),
        )
    }

    pub fn try_clone(&self) -> Result<Self, InsertError> {
        Ok(Self {
            entries: try_clone_copy_entries(&self.entries, StorageResource::AssignmentEntries)?,
        })
    }

    pub(crate) fn entries(&self) -> &[(ConsumerId, ExprId)] {
        &self.entries
    }
}

#[allow(clippy::len_without_is_empty, clippy::new_without_default)]
impl EvaluationCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, id: ExprId) -> Option<&Result<ExactValue, EvalError>> {
        get_entry(&self.entries, id)
    }

    pub(crate) fn get_mut(&mut self, id: ExprId) -> Option<&mut Result<ExactValue, EvalError>> {
        self.entries
            .binary_search_by(|(candidate, _)| candidate.cmp(&id))
            .ok()
            .map(|index| &mut self.entries[index].1)
    }

    pub(crate) fn insert(
        &mut self,
        id: ExprId,
        value: Result<ExactValue, EvalError>,
    ) -> Result<(), StorageError> {
        let index = match find_key(&self.entries, &id) {
            Ok(_) => return Ok(()),
            Err(index) => index,
        };
        let required = required_elements(self.entries.len(), StorageResource::EvaluationEntries)?;
        reserve_entries(
            &mut self.entries,
            required,
            StorageResource::EvaluationEntries,
        )?;
        self.entries.insert(index, (id, value));
        Ok(())
    }
}

#[allow(clippy::len_without_is_empty)]
impl IsolationCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, id: ExprId, bits: AbsoluteBits) -> Option<&IsolatingInterval> {
        get_entry(&self.entries, (id, bits))
    }

    pub(crate) fn insert(
        &mut self,
        id: ExprId,
        bits: AbsoluteBits,
        interval: IsolatingInterval,
    ) -> Result<(), StorageError> {
        insert_cache_entry(
            &mut self.entries,
            (id, bits),
            interval,
            StorageResource::IsolationEntries,
        )
    }
}

#[allow(clippy::len_without_is_empty)]
impl ResolvedValues {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, id: ConsumerId) -> Option<&Result<CertifiedF64, ResolveError>> {
        get_entry(&self.entries, id)
    }

    pub(crate) fn insert(
        &mut self,
        id: ConsumerId,
        value: Result<CertifiedF64, ResolveError>,
    ) -> Result<(), StorageError> {
        insert_cache_entry(
            &mut self.entries,
            id,
            value,
            StorageResource::ResolvedEntries,
        )
    }
}

fn insert_cache_entry<K: Ord, V>(
    entries: &mut Vec<(K, V)>,
    key: K,
    value: V,
    resource: StorageResource,
) -> Result<(), StorageError> {
    let index = match find_key(entries, &key) {
        Ok(_) => return Ok(()),
        Err(index) => index,
    };
    let required = required_elements(entries.len(), resource)?;
    reserve_entries(entries, required, resource)?;
    entries.insert(index, (key, value));
    Ok(())
}

fn find_key<K: Ord, V>(entries: &[(K, V)], key: &K) -> Result<usize, usize> {
    entries.binary_search_by(|(candidate, _)| candidate.cmp(key))
}

fn get_entry<K: Ord, V>(entries: &[(K, V)], key: K) -> Option<&V> {
    find_key(entries, &key).ok().map(|index| &entries[index].1)
}

fn insert_entry<K: Ord, V>(
    entries: &mut Vec<(K, V)>,
    key: K,
    value: V,
    resource: StorageResource,
    id: u32,
) -> Result<(), InsertError> {
    let index = match find_key(entries, &key) {
        Ok(_) => return Err(InsertError::DuplicateId { resource, id }),
        Err(index) => index,
    };
    let required = required_elements(entries.len(), resource)?;
    reserve_entries(entries, required, resource)?;
    entries.insert(index, (key, value));
    Ok(())
}

fn set_entry<K: Ord, V>(
    entries: &mut Vec<(K, V)>,
    key: K,
    value: V,
    resource: StorageResource,
    id: u32,
) -> Result<(), InsertError> {
    match find_key(entries, &key) {
        Ok(index) => {
            entries[index].1 = value;
            Ok(())
        }
        Err(_) => insert_entry(entries, key, value, resource, id),
    }
}

fn try_clone_copy_entries<K: Copy, V: Copy>(
    source: &[(K, V)],
    resource: StorageResource,
) -> Result<Vec<(K, V)>, InsertError> {
    let mut entries = Vec::new();
    reserve_entries(&mut entries, source.len(), resource)?;
    entries.extend_from_slice(source);
    Ok(entries)
}

pub(crate) fn required_elements(
    current: usize,
    resource: StorageResource,
) -> Result<usize, StorageError> {
    current
        .checked_add(1)
        .ok_or(StorageError::CapacityOverflow { resource })
}

pub(crate) fn reserve_entries<T>(
    entries: &mut Vec<T>,
    total_required: usize,
    resource: StorageResource,
) -> Result<(), StorageError> {
    #[cfg(test)]
    if let Some(error) = injected_failure(resource, total_required) {
        return Err(error);
    }

    let maximum = if size_of::<T>() == 0 {
        usize::MAX
    } else {
        (isize::MAX as usize) / size_of::<T>()
    };
    if total_required > maximum {
        return Err(StorageError::CapacityOverflow { resource });
    }
    let additional = total_required
        .checked_sub(entries.len())
        .ok_or(StorageError::CapacityOverflow { resource })?;
    entries
        .try_reserve(additional)
        .map_err(|_| StorageError::AllocationFailure {
            resource,
            requested_elements: total_required,
        })
}

#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) enum InjectedFailure {
    Capacity,
    Allocation,
}

#[cfg(test)]
std::thread_local! {
    static INJECTED_FAILURE: core::cell::Cell<Option<(StorageResource, InjectedFailure)>> = const { core::cell::Cell::new(None) };
}

#[cfg(test)]
fn injected_failure(resource: StorageResource, requested_elements: usize) -> Option<StorageError> {
    INJECTED_FAILURE.with(|configured| {
        let error = match configured.get() {
            Some((expected, InjectedFailure::Capacity)) if expected == resource => {
                Some(StorageError::CapacityOverflow { resource })
            }
            Some((expected, InjectedFailure::Allocation)) if expected == resource => {
                Some(StorageError::AllocationFailure {
                    resource,
                    requested_elements,
                })
            }
            _ => None,
        };
        if error.is_some() {
            configured.set(None);
        }
        error
    })
}

#[cfg(test)]
pub(crate) fn with_injected_failure<R>(
    resource: StorageResource,
    failure: InjectedFailure,
    operation: impl FnOnce() -> R,
) -> R {
    INJECTED_FAILURE.with(|configured| configured.set(Some((resource, failure))));
    let result = operation();
    INJECTED_FAILURE.with(|configured| configured.set(None));
    result
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use neco_algnum::RealAlgebraic;
    use neco_formsum::FormSum;
    use neco_monomial::{Monomial, MonomialErrorKind};

    use super::{
        required_elements, reserve_entries, with_injected_failure, InjectedFailure, StorageResource,
    };
    use crate::value::{with_clone_failure, CloneContact};
    use crate::{
        AbsoluteBits, Assignments, AtomId, AtomStore, ConsumerId, EvalError, ExactValue, ExprGraph,
        ExprNode, GraphError, InsertError, PrecisionRequirements, ResolveError, Resolver,
        StorageError,
    };

    const RESOURCES: [StorageResource; 7] = [
        StorageResource::GraphNodes,
        StorageResource::AtomEntries,
        StorageResource::PrecisionEntries,
        StorageResource::AssignmentEntries,
        StorageResource::EvaluationEntries,
        StorageResource::IsolationEntries,
        StorageResource::ResolvedEntries,
    ];

    #[test]
    fn required_total_rejects_capacity_overflow() {
        for resource in RESOURCES {
            assert_eq!(
                required_elements(usize::MAX, resource),
                Err(StorageError::CapacityOverflow { resource })
            );
        }
    }

    #[test]
    fn allocation_failure_reports_total_and_preserves_entries() {
        let mut entries = vec![10_u32];
        let result = with_injected_failure(
            StorageResource::PrecisionEntries,
            InjectedFailure::Allocation,
            || reserve_entries(&mut entries, 2, StorageResource::PrecisionEntries),
        );
        assert_eq!(
            result,
            Err(StorageError::AllocationFailure {
                resource: StorageResource::PrecisionEntries,
                requested_elements: 2,
            })
        );
        assert_eq!(entries, [10]);
    }

    #[test]
    fn capacity_injection_uses_production_reservation_path() {
        let mut requirements = PrecisionRequirements::new();
        let result = with_injected_failure(
            StorageResource::PrecisionEntries,
            InjectedFailure::Capacity,
            || requirements.insert(ConsumerId::new(1), AbsoluteBits::new(20)),
        );
        assert_eq!(
            result,
            Err(InsertError::Storage(StorageError::CapacityOverflow {
                resource: StorageResource::PrecisionEntries,
            }))
        );
        assert_eq!(requirements.len(), 0);
    }

    #[test]
    fn allocation_injection_identifies_every_resource() {
        for resource in RESOURCES {
            let mut entries = Vec::<u32>::new();
            let result = with_injected_failure(resource, InjectedFailure::Allocation, || {
                reserve_entries(&mut entries, 1, resource)
            });
            assert_eq!(
                result,
                Err(StorageError::AllocationFailure {
                    resource,
                    requested_elements: 1,
                })
            );
            assert!(entries.is_empty());
        }
    }

    #[test]
    fn map_order_duplicate_and_set_are_deterministic() {
        let mut left = PrecisionRequirements::new();
        left.insert(ConsumerId::new(2), AbsoluteBits::new(20))
            .unwrap();
        left.insert(ConsumerId::new(1), AbsoluteBits::new(10))
            .unwrap();

        let mut right = PrecisionRequirements::new();
        right
            .insert(ConsumerId::new(1), AbsoluteBits::new(10))
            .unwrap();
        right
            .insert(ConsumerId::new(2), AbsoluteBits::new(20))
            .unwrap();
        assert_eq!(left, right);

        assert_eq!(
            left.insert(ConsumerId::new(1), AbsoluteBits::new(99)),
            Err(InsertError::DuplicateId {
                resource: StorageResource::PrecisionEntries,
                id: 1,
            })
        );
        assert_eq!(left.get(ConsumerId::new(1)), Some(AbsoluteBits::new(10)));

        left.set(ConsumerId::new(1), AbsoluteBits::new(30)).unwrap();
        assert_eq!(left.get(ConsumerId::new(1)), Some(AbsoluteBits::new(30)));
        assert_eq!(left.len(), 2);
    }

    #[test]
    fn graph_storage_failure_is_wrapped_and_preserves_graph() {
        let mut graph = ExprGraph::new();
        let result = with_injected_failure(
            StorageResource::GraphNodes,
            InjectedFailure::Allocation,
            || graph.push(ExprNode::Atom(AtomId::new(0))),
        );
        assert_eq!(
            result,
            Err(GraphError::Storage(StorageError::AllocationFailure {
                resource: StorageResource::GraphNodes,
                requested_elements: 1,
            }))
        );
        assert!(graph.is_empty());
    }

    #[test]
    fn every_input_map_wraps_allocation_failure_and_preserves_its_value() {
        let mut atoms = AtomStore::new();
        let atom_result = with_injected_failure(
            StorageResource::AtomEntries,
            InjectedFailure::Allocation,
            || atoms.insert(AtomId::new(4), ExactValue::Monomial(Monomial::one())),
        );
        assert_eq!(
            atom_result,
            Err(InsertError::Storage(StorageError::AllocationFailure {
                resource: StorageResource::AtomEntries,
                requested_elements: 1,
            }))
        );
        assert_eq!(atoms.len(), 0);

        let mut requirements = PrecisionRequirements::new();
        let precision_result = with_injected_failure(
            StorageResource::PrecisionEntries,
            InjectedFailure::Allocation,
            || requirements.insert(ConsumerId::new(4), AbsoluteBits::new(20)),
        );
        assert_eq!(
            precision_result,
            Err(InsertError::Storage(StorageError::AllocationFailure {
                resource: StorageResource::PrecisionEntries,
                requested_elements: 1,
            }))
        );
        assert_eq!(requirements.len(), 0);

        let mut assignments = Assignments::new();
        let assignment_result = with_injected_failure(
            StorageResource::AssignmentEntries,
            InjectedFailure::Allocation,
            || assignments.insert(ConsumerId::new(4), crate::ExprId::new(0)),
        );
        assert_eq!(
            assignment_result,
            Err(InsertError::Storage(StorageError::AllocationFailure {
                resource: StorageResource::AssignmentEntries,
                requested_elements: 1,
            }))
        );
        assert_eq!(assignments.len(), 0);
    }

    #[test]
    fn atom_clone_failure_is_value_error_with_the_original_payload() {
        let mut atoms = AtomStore::new();
        atoms
            .insert(AtomId::new(0), ExactValue::Monomial(Monomial::one()))
            .unwrap();
        assert_eq!(
            with_clone_failure(CloneContact::Monomial, || atoms.try_clone()),
            Err(InsertError::Value(EvalError::Monomial(
                MonomialErrorKind::AllocationFailure {
                    requested_elements: 19,
                }
            )))
        );
        assert_eq!(atoms.len(), 1);
    }

    #[test]
    fn evaluation_clone_failure_is_local_and_preserves_the_cache_entry() {
        let mut graph = ExprGraph::new();
        let failed = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let succeeded = graph.push(ExprNode::Atom(AtomId::new(1))).unwrap();
        let mut atoms = AtomStore::new();
        atoms
            .insert(AtomId::new(0), ExactValue::Monomial(Monomial::one()))
            .unwrap();
        atoms
            .insert(AtomId::new(1), ExactValue::FormSum(FormSum::one().unwrap()))
            .unwrap();
        let mut requirements = PrecisionRequirements::new();
        let mut assignments = Assignments::new();
        for (consumer, expression) in [
            (ConsumerId::new(0), failed),
            (ConsumerId::new(1), succeeded),
        ] {
            requirements.insert(consumer, AbsoluteBits::new(0)).unwrap();
            assignments.insert(consumer, expression).unwrap();
        }
        let (evaluation, _, resolved) = with_clone_failure(CloneContact::Monomial, || {
            Resolver::new().resolve_all(&graph, &atoms, &requirements, &assignments)
        })
        .unwrap();
        let expected = EvalError::Monomial(MonomialErrorKind::AllocationFailure {
            requested_elements: 19,
        });
        assert_eq!(evaluation.get(failed), Some(&Err(expected)));
        assert_eq!(
            resolved.get(ConsumerId::new(0)),
            Some(&Err(ResolveError::Evaluation(EvalError::Monomial(
                MonomialErrorKind::AllocationFailure {
                    requested_elements: 19,
                }
            ))))
        );
        assert!(resolved.get(ConsumerId::new(1)).unwrap().as_ref().is_ok());
    }

    #[test]
    fn resolver_reports_evaluation_and_result_storage_failures_globally() {
        let mut graph = ExprGraph::new();
        let expr = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let mut atoms = AtomStore::new();
        atoms
            .insert(AtomId::new(0), ExactValue::Monomial(Monomial::one()))
            .unwrap();
        let mut requirements = PrecisionRequirements::new();
        requirements
            .insert(ConsumerId::new(0), AbsoluteBits::new(0))
            .unwrap();
        let mut assignments = Assignments::new();
        assignments.insert(ConsumerId::new(0), expr).unwrap();

        for resource in [
            StorageResource::EvaluationEntries,
            StorageResource::ResolvedEntries,
        ] {
            let result = with_injected_failure(resource, InjectedFailure::Allocation, || {
                Resolver::new().resolve_all(&graph, &atoms, &requirements, &assignments)
            });
            assert_eq!(
                result,
                Err(ResolveError::Storage(StorageError::AllocationFailure {
                    resource,
                    requested_elements: 1,
                }))
            );
        }
    }

    #[test]
    fn resolver_reports_isolation_storage_failure_globally() {
        let form = FormSum::one().unwrap();
        let mut graph = ExprGraph::new();
        let expr = graph.push(ExprNode::Atom(AtomId::new(0))).unwrap();
        let mut atoms = AtomStore::new();
        atoms
            .insert(
                AtomId::new(0),
                ExactValue::Algebraic(RealAlgebraic::from_form_sum(&form).unwrap()),
            )
            .unwrap();
        let mut requirements = PrecisionRequirements::new();
        requirements
            .insert(ConsumerId::new(0), AbsoluteBits::new(0))
            .unwrap();
        let mut assignments = Assignments::new();
        assignments.insert(ConsumerId::new(0), expr).unwrap();

        let result = with_injected_failure(
            StorageResource::IsolationEntries,
            InjectedFailure::Allocation,
            || Resolver::new().resolve_all(&graph, &atoms, &requirements, &assignments),
        );
        assert_eq!(
            result,
            Err(ResolveError::Storage(StorageError::AllocationFailure {
                resource: StorageResource::IsolationEntries,
                requested_elements: 1,
            }))
        );
    }
}
