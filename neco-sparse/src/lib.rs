#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::vec::Vec;
use core::ops::AddAssign;
use neco_linear_types::{ColumnIndex, LinearError, LinearOperator, RowIndex, Shape, Vector};

#[derive(Clone, Debug, PartialEq)]
struct CooEntry<T> {
    row: usize,
    column: usize,
    value: T,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CooMatrix<T> {
    shape: Shape,
    entries: Vec<CooEntry<T>>,
}

impl<T> CooMatrix<T> {
    pub fn new(shape: Shape) -> Self {
        Self {
            shape,
            entries: Vec::new(),
        }
    }

    pub fn shape(&self) -> Shape {
        self.shape
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push(
        &mut self,
        row: RowIndex,
        column: ColumnIndex,
        value: T,
    ) -> Result<(), LinearError> {
        validate_indices(self.shape, row, column)?;
        let requested = self
            .entries
            .len()
            .checked_add(1)
            .ok_or(LinearError::CapacityOverflow {
                requested: usize::MAX,
            })?;
        self.entries
            .try_reserve(1)
            .map_err(|_| LinearError::AllocationFailure { requested })?;
        self.entries.push(CooEntry {
            row: row.value(),
            column: column.value(),
            value,
        });
        Ok(())
    }

    pub fn to_csr(mut self) -> Result<CsrMatrix<T>, LinearError>
    where
        T: AddAssign,
    {
        for entry_position in 1..self.entries.len() {
            let mut insertion_position = entry_position;
            while insertion_position > 0 {
                let previous_position = insertion_position - 1;
                let entry = &self.entries[insertion_position];
                let previous = &self.entries[previous_position];
                if (entry.row, entry.column) >= (previous.row, previous.column) {
                    break;
                }
                self.entries.swap(insertion_position, previous_position);
                insertion_position = previous_position;
            }
        }
        let mut entries: Vec<CooEntry<T>> = Vec::new();
        entries.try_reserve_exact(self.entries.len()).map_err(|_| {
            LinearError::AllocationFailure {
                requested: self.entries.len(),
            }
        })?;
        for entry in self.entries {
            if let Some(previous) = entries.last_mut() {
                if previous.row == entry.row && previous.column == entry.column {
                    previous.value += entry.value;
                    continue;
                }
            }
            entries.push(entry);
        }
        CsrMatrix::from_sorted_entries(self.shape, entries)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CsrMatrix<T> {
    shape: Shape,
    row_offsets: Vec<usize>,
    column_indices: Vec<usize>,
    values: Vec<T>,
}

impl<T> CsrMatrix<T> {
    pub fn from_parts(
        shape: Shape,
        row_offsets: Vec<usize>,
        column_indices: Vec<usize>,
        values: Vec<T>,
    ) -> Result<Self, LinearError> {
        validate_csr_parts(shape, &row_offsets, &column_indices, values.len())?;
        Ok(Self {
            shape,
            row_offsets,
            column_indices,
            values,
        })
    }

    pub fn shape(&self) -> Shape {
        self.shape
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn row(&self, row: RowIndex) -> Result<CsrRow<'_, T>, LinearError> {
        if row.value() >= self.shape.rows() {
            return Err(LinearError::IndexOutOfBounds {
                axis: "row",
                index: row.value(),
                bound: self.shape.rows(),
            });
        }
        let start = self.row_offsets[row.value()];
        let end = self.row_offsets[row.value() + 1];
        Ok(CsrRow {
            column_indices: &self.column_indices[start..end],
            values: &self.values[start..end],
        })
    }

    pub fn row_offsets(&self) -> &[usize] {
        &self.row_offsets
    }

    pub fn column_indices(&self) -> &[usize] {
        &self.column_indices
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }

    fn from_sorted_entries(shape: Shape, entries: Vec<CooEntry<T>>) -> Result<Self, LinearError> {
        let row_offset_count =
            shape
                .rows()
                .checked_add(1)
                .ok_or(LinearError::CapacityOverflow {
                    requested: usize::MAX,
                })?;
        let entry_count = entries.len();
        let mut row_offsets: Vec<usize> = Vec::new();
        row_offsets
            .try_reserve_exact(row_offset_count)
            .map_err(|_| LinearError::AllocationFailure {
                requested: row_offset_count,
            })?;
        row_offsets.push(0);
        let mut entry_position = 0;
        for row in 0..shape.rows() {
            while entry_position < entry_count && entries[entry_position].row == row {
                entry_position =
                    entry_position
                        .checked_add(1)
                        .ok_or(LinearError::CapacityOverflow {
                            requested: usize::MAX,
                        })?;
            }
            row_offsets.push(entry_position);
        }
        let mut column_indices = Vec::new();
        let mut values = Vec::new();
        column_indices.try_reserve_exact(entry_count).map_err(|_| {
            LinearError::AllocationFailure {
                requested: entry_count,
            }
        })?;
        values
            .try_reserve_exact(entry_count)
            .map_err(|_| LinearError::AllocationFailure {
                requested: entry_count,
            })?;
        for entry in entries {
            column_indices.push(entry.column);
            values.push(entry.value);
        }
        Self::from_parts(shape, row_offsets, column_indices, values)
    }
}

pub struct CsrRow<'a, T> {
    column_indices: &'a [usize],
    values: &'a [T],
}

impl<'a, T> CsrRow<'a, T> {
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn column_indices(&self) -> &'a [usize] {
        self.column_indices
    }

    pub fn values(&self) -> &'a [T] {
        self.values
    }

    pub fn entries(&self) -> impl Iterator<Item = (usize, &'a T)> + '_ {
        self.column_indices.iter().copied().zip(self.values.iter())
    }
}

impl LinearOperator<f64> for CsrMatrix<f64> {
    fn domain(&self) -> usize {
        self.shape.columns()
    }

    fn codomain(&self) -> usize {
        self.shape.rows()
    }

    fn apply(&self, input: &Vector<f64>) -> Result<Vector<f64>, LinearError> {
        if input.len() != self.domain() {
            return Err(LinearError::StorageLengthMismatch {
                expected: self.domain(),
                actual: input.len(),
            });
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(self.codomain())
            .map_err(|_| LinearError::AllocationFailure {
                requested: self.codomain(),
            })?;
        output.resize(self.codomain(), 0.0);
        for (row, output_value) in output.iter_mut().enumerate() {
            let start = self.row_offsets[row];
            let end = self.row_offsets[row + 1];
            for position in start..end {
                let column = self.column_indices[position];
                let input_value = *input.value(column)?;
                *output_value += self.values[position] * input_value;
            }
        }
        Vector::try_from_vec(output)
    }
}

fn validate_indices(shape: Shape, row: RowIndex, column: ColumnIndex) -> Result<(), LinearError> {
    if row.value() >= shape.rows() {
        return Err(LinearError::IndexOutOfBounds {
            axis: "row",
            index: row.value(),
            bound: shape.rows(),
        });
    }
    if column.value() >= shape.columns() {
        return Err(LinearError::IndexOutOfBounds {
            axis: "column",
            index: column.value(),
            bound: shape.columns(),
        });
    }
    Ok(())
}

fn validate_csr_parts(
    shape: Shape,
    row_offsets: &[usize],
    column_indices: &[usize],
    values_len: usize,
) -> Result<(), LinearError> {
    let expected_offsets = shape
        .rows()
        .checked_add(1)
        .ok_or(LinearError::CapacityOverflow {
            requested: usize::MAX,
        })?;
    if row_offsets.len() != expected_offsets {
        return Err(LinearError::InvalidStorage {
            reason: "CSR row offset length must equal rows plus one",
        });
    }
    if row_offsets.first().copied() != Some(0) {
        return Err(LinearError::InvalidStorage {
            reason: "CSR row offsets must start at zero",
        });
    }
    if column_indices.len() != values_len {
        return Err(LinearError::InvalidStorage {
            reason: "CSR column and value lengths must match",
        });
    }
    if row_offsets.last().copied() != Some(values_len) {
        return Err(LinearError::InvalidStorage {
            reason: "CSR final row offset must equal the value length",
        });
    }
    if row_offsets.windows(2).any(|window| window[0] > window[1]) {
        return Err(LinearError::InvalidStorage {
            reason: "CSR row offsets must be monotonic",
        });
    }
    if column_indices
        .iter()
        .any(|&column| column >= shape.columns())
    {
        return Err(LinearError::InvalidStorage {
            reason: "CSR column index is outside the matrix shape",
        });
    }
    for row in 0..shape.rows() {
        let start = row_offsets[row];
        let end = row_offsets[row + 1];
        if column_indices[start..end]
            .windows(2)
            .any(|window| window[0] >= window[1])
        {
            return Err(LinearError::InvalidStorage {
                reason: "CSR columns must be strictly increasing within each row",
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CooMatrix, CsrMatrix};
    use alloc::vec;
    use neco_linear_types::{LinearError, LinearOperator, Shape, Vector};

    #[test]
    fn coo_permutation_produces_the_same_csr() {
        let shape = Shape::new(2, 3);
        let entries = [(0, 2, 4), (0, 0, 1), (1, 1, 3)];
        let mut first = CooMatrix::new(shape);
        let mut second = CooMatrix::new(shape);
        for &(row, column, value) in &entries {
            first
                .push(
                    shape.row_index(row).unwrap(),
                    shape.column_index(column).unwrap(),
                    value,
                )
                .unwrap();
        }
        for &(row, column, value) in entries.iter().rev() {
            second
                .push(
                    shape.row_index(row).unwrap(),
                    shape.column_index(column).unwrap(),
                    value,
                )
                .unwrap();
        }
        assert_eq!(first.to_csr().unwrap(), second.to_csr().unwrap());
    }

    #[test]
    fn coo_duplicate_coordinates_are_added() {
        let shape = Shape::new(1, 2);
        let mut matrix = CooMatrix::new(shape);
        matrix
            .push(
                shape.row_index(0).unwrap(),
                shape.column_index(1).unwrap(),
                2,
            )
            .unwrap();
        matrix
            .push(
                shape.row_index(0).unwrap(),
                shape.column_index(1).unwrap(),
                5,
            )
            .unwrap();
        let csr = matrix.to_csr().unwrap();
        assert_eq!(csr.values(), &[7]);
        assert_eq!(csr.column_indices(), &[1]);
    }

    #[test]
    fn csr_rejects_invalid_storage() {
        let result = CsrMatrix::from_parts(Shape::new(1, 2), vec![0, 1], vec![1, 2], vec![3, 4]);
        assert!(matches!(result, Err(LinearError::InvalidStorage { .. })));
    }

    #[test]
    fn coo_to_csr_preserves_shape_and_values() {
        let shape = Shape::new(2, 3);
        let mut matrix = CooMatrix::new(shape);
        matrix
            .push(
                shape.row_index(1).unwrap(),
                shape.column_index(2).unwrap(),
                6,
            )
            .unwrap();
        matrix
            .push(
                shape.row_index(0).unwrap(),
                shape.column_index(0).unwrap(),
                1,
            )
            .unwrap();
        let csr = matrix.to_csr().unwrap();
        assert_eq!(csr.shape(), shape);
        assert_eq!(csr.row_offsets(), &[0, 1, 2]);
        assert_eq!(csr.column_indices(), &[0, 2]);
        assert_eq!(csr.values(), &[1, 6]);
    }

    #[test]
    fn csr_matrix_vector_product_preserves_values() {
        let csr = CsrMatrix::from_parts(
            Shape::new(2, 3),
            vec![0, 2, 3],
            vec![0, 2, 1],
            vec![1.0, 3.0, 4.0],
        )
        .unwrap();
        let input = Vector::try_from_vec(vec![10.0, 20.0, 30.0]).unwrap();
        assert_eq!(csr.apply(&input).unwrap().values(), &[100.0, 80.0]);
    }

    #[test]
    fn csr_matrix_vector_product_rejects_short_vector() {
        let csr = CsrMatrix::from_parts(Shape::new(2, 3), vec![0, 0, 0], vec![], vec![]).unwrap();
        let input = Vector::try_from_vec(vec![1.0, 2.0]).unwrap();
        assert!(matches!(
            csr.apply(&input),
            Err(LinearError::StorageLengthMismatch {
                expected: 3,
                actual: 2
            })
        ));
    }
}
