use alloc::vec::Vec;

use crate::storage::{reserve_entries, StorageResource};
use crate::StorageError;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SortedIndex<K> {
    entries: Vec<(K, usize)>,
}

impl<K> Default for SortedIndex<K> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<K: Copy + Ord> SortedIndex<K> {
    pub(crate) fn build<I>(keys: I, resource: StorageResource) -> Result<Self, StorageError>
    where
        I: ExactSizeIterator<Item = K>,
    {
        let mut entries = Vec::new();
        if keys.len() == 0 {
            return Ok(Self { entries });
        }
        reserve_entries(&mut entries, keys.len(), resource)?;
        entries.extend(keys.enumerate().map(|(position, key)| (key, position)));
        entries.sort_unstable();
        Ok(Self { entries })
    }

    fn lower_bound(&self, key: &K) -> usize {
        self.entries.partition_point(|(entry, _)| entry < key)
    }

    pub(crate) fn first(&self, key: &K) -> Option<usize> {
        self.entries
            .get(self.lower_bound(key))
            .filter(|(entry, _)| entry == key)
            .map(|(_, position)| *position)
    }

    pub(crate) fn count(&self, key: &K) -> usize {
        let lower = self.lower_bound(key);
        self.entries[lower..].partition_point(|(entry, _)| entry == key)
    }

    pub(crate) fn second_occurrences(&self) -> impl Iterator<Item = usize> + '_ {
        self.entries
            .windows(2)
            .filter(|pair| pair[0].0 == pair[1].0)
            .map(|pair| pair[1].1)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct GrowingIndex<K> {
    runs: Vec<SortedIndex<K>>,
}

impl<K> Default for GrowingIndex<K> {
    fn default() -> Self {
        Self { runs: Vec::new() }
    }
}

impl<K: Copy + Ord> GrowingIndex<K> {
    pub(crate) fn get(&self, key: &K) -> Option<usize> {
        self.runs.iter().find_map(|run| run.first(key))
    }

    pub(crate) fn insert(
        &mut self,
        key: K,
        position: usize,
        resource: StorageResource,
    ) -> Result<(), StorageError> {
        let level = self
            .runs
            .iter()
            .position(|run| run.entries.is_empty())
            .unwrap_or(self.runs.len());
        let total = 1_usize
            .checked_shl(level as u32)
            .ok_or(StorageError::CapacityOverflow { resource })?;
        if level == self.runs.len() {
            let required = level
                .checked_add(1)
                .ok_or(StorageError::CapacityOverflow { resource })?;
            reserve_entries(&mut self.runs, required, resource)?;
        }
        let mut merged = Vec::new();
        reserve_entries(&mut merged, total, resource)?;
        merged.push((key, position));
        for run in &self.runs[..level] {
            merged.extend_from_slice(&run.entries);
        }
        merged.sort_unstable();
        for run in &mut self.runs[..level] {
            *run = SortedIndex::default();
        }
        let merged = SortedIndex { entries: merged };
        if level == self.runs.len() {
            self.runs.push(merged);
        } else {
            self.runs[level] = merged;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_index_returns_first_position_and_count_of_duplicates() {
        let keys = [(1, 'a'), (2, 'b'), (1, 'c'), (1, 'a'), (2, 'b')];
        let index = SortedIndex::build(keys.into_iter(), StorageResource::ProductEntries).unwrap();
        assert_eq!(index.first(&(1, 'a')), Some(0));
        assert_eq!(index.count(&(1, 'a')), 2);
        assert_eq!(index.first(&(2, 'b')), Some(1));
        assert_eq!(index.count(&(2, 'b')), 2);
        assert_eq!(index.first(&(1, 'c')), Some(2));
        assert_eq!(index.first(&(3, 'a')), None);
        assert_eq!(index.count(&(3, 'a')), 0);
        assert_eq!(index.second_occurrences().min(), Some(3));
    }

    #[test]
    fn growing_index_finds_every_inserted_key() {
        let mut index = GrowingIndex::default();
        for position in 0..200_usize {
            let key = (position * 7919) % 211;
            assert_eq!(index.get(&key), None);
            index
                .insert(key, position, StorageResource::ProductEntries)
                .unwrap();
        }
        for position in 0..200_usize {
            assert_eq!(index.get(&((position * 7919) % 211)), Some(position));
        }
        assert_eq!(index.get(&500), None);
    }
}
