#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::vec::Vec;
use neco_linear_types::{ColumnIndex, LinearError, LinearOperator, RowIndex, Shape, Vector};

#[derive(Clone, Debug, PartialEq)]
pub struct DenseMatrix<T> {
    shape: Shape,
    values: Vec<T>,
}

impl<T> DenseMatrix<T> {
    pub fn from_column_major(shape: Shape, values: Vec<T>) -> Result<Self, LinearError> {
        let expected = shape.element_count()?;
        if values.len() != expected {
            return Err(LinearError::StorageLengthMismatch {
                expected,
                actual: values.len(),
            });
        }
        if values.len() > isize::MAX as usize {
            return Err(LinearError::InvalidStorage {
                reason: "matrix length exceeds the representable allocation limit",
            });
        }
        Ok(Self { shape, values })
    }

    pub fn from_row_major(shape: Shape, values: Vec<T>) -> Result<Self, LinearError>
    where
        T: Clone,
    {
        let expected = shape.element_count()?;
        if values.len() != expected {
            return Err(LinearError::StorageLengthMismatch {
                expected,
                actual: values.len(),
            });
        }
        let mut column_major = Vec::new();
        column_major
            .try_reserve_exact(expected)
            .map_err(|_| LinearError::AllocationFailure {
                requested: expected,
            })?;
        for column in 0..shape.columns() {
            for row in 0..shape.rows() {
                let row_major_index = row
                    .checked_mul(shape.columns())
                    .and_then(|offset| offset.checked_add(column))
                    .ok_or(LinearError::CapacityOverflow {
                        requested: usize::MAX,
                    })?;
                column_major.push(values[row_major_index].clone());
            }
        }
        Self::from_column_major(shape, column_major)
    }

    pub fn shape(&self) -> Shape {
        self.shape
    }

    pub fn rows(&self) -> usize {
        self.shape.rows()
    }

    pub fn columns(&self) -> usize {
        self.shape.columns()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn value(&self, row: RowIndex, column: ColumnIndex) -> Result<&T, LinearError> {
        if row.value() >= self.shape.rows() {
            return Err(LinearError::IndexOutOfBounds {
                axis: "row",
                index: row.value(),
                bound: self.shape.rows(),
            });
        }
        if column.value() >= self.shape.columns() {
            return Err(LinearError::IndexOutOfBounds {
                axis: "column",
                index: column.value(),
                bound: self.shape.columns(),
            });
        }
        let position = column
            .value()
            .checked_mul(self.shape.rows())
            .and_then(|offset| offset.checked_add(row.value()))
            .ok_or(LinearError::CapacityOverflow {
                requested: usize::MAX,
            })?;
        self.values
            .get(position)
            .ok_or(LinearError::InvalidStorage {
                reason: "matrix storage is shorter than its declared shape",
            })
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }

    pub fn into_values(self) -> Vec<T> {
        self.values
    }
}

impl<T> DenseMatrix<T>
where
    T: Clone,
{
    pub fn try_zeros(shape: Shape, value: T) -> Result<Self, LinearError> {
        let length = shape.element_count()?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| LinearError::AllocationFailure { requested: length })?;
        values.resize(length, value);
        Self::from_column_major(shape, values)
    }
}

impl LinearOperator<f64> for DenseMatrix<f64> {
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
        let mut output_values = Vec::new();
        output_values
            .try_reserve_exact(self.codomain())
            .map_err(|_| LinearError::AllocationFailure {
                requested: self.codomain(),
            })?;
        output_values.resize(self.codomain(), 0.0);
        for column in 0..self.domain() {
            let input_value = *input.value(column)?;
            let column_offset =
                column
                    .checked_mul(self.shape.rows())
                    .ok_or(LinearError::CapacityOverflow {
                        requested: usize::MAX,
                    })?;
            for (row, output_value) in output_values.iter_mut().enumerate() {
                let position =
                    column_offset
                        .checked_add(row)
                        .ok_or(LinearError::CapacityOverflow {
                            requested: usize::MAX,
                        })?;
                let matrix_value =
                    *self
                        .values
                        .get(position)
                        .ok_or(LinearError::InvalidStorage {
                            reason: "matrix storage is shorter than its declared shape",
                        })?;
                *output_value += matrix_value * input_value;
            }
        }
        Vector::try_from_vec(output_values)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::DenseMatrix;
    use neco_linear_types::{LinearError, LinearOperator, Shape, Vector};

    #[test]
    fn row_major_input_is_stored_and_observed_in_column_major_order() {
        let shape = Shape::new(2, 3);
        let matrix =
            DenseMatrix::from_row_major(shape, vec![1, 2, 3, 4, 5, 6]).expect("valid matrix");
        assert_eq!(matrix.values(), &[1, 4, 2, 5, 3, 6]);
        assert_eq!(
            matrix.value(
                shape.row_index(1).expect("row"),
                shape.column_index(2).expect("column")
            ),
            Ok(&6)
        );
    }

    #[test]
    fn storage_length_mismatch_is_reported() {
        let result = DenseMatrix::from_column_major(Shape::new(2, 2), vec![1, 2, 3]);
        assert!(matches!(
            result,
            Err(LinearError::StorageLengthMismatch {
                expected: 4,
                actual: 3
            })
        ));
    }

    #[test]
    fn value_rejects_row_index_from_a_different_shape() {
        let matrix_shape = Shape::new(2, 2);
        let row = Shape::new(3, 1)
            .row_index(2)
            .expect("valid row in source shape");
        let column = matrix_shape.column_index(0).expect("valid column");
        let matrix = DenseMatrix::try_zeros(matrix_shape, 0).expect("valid matrix");

        assert!(matches!(
            matrix.value(row, column),
            Err(LinearError::IndexOutOfBounds {
                axis: "row",
                index: 2,
                bound: 2
            })
        ));
    }

    #[test]
    fn value_rejects_column_index_from_a_different_shape() {
        let matrix_shape = Shape::new(2, 2);
        let row = matrix_shape.row_index(0).expect("valid row");
        let column = Shape::new(1, 3)
            .column_index(2)
            .expect("valid column in source shape");
        let matrix = DenseMatrix::try_zeros(matrix_shape, 0).expect("valid matrix");

        assert!(matches!(
            matrix.value(row, column),
            Err(LinearError::IndexOutOfBounds {
                axis: "column",
                index: 2,
                bound: 2
            })
        ));
    }

    #[test]
    fn matrix_vector_product_preserves_values_and_dimensions() {
        let matrix = DenseMatrix::from_row_major(Shape::new(2, 2), vec![1.0, 2.0, 3.0, 4.0])
            .expect("valid matrix");
        let input = Vector::try_from_vec(vec![10.0, 20.0]).expect("valid vector");
        assert_eq!(matrix.domain(), 2);
        assert_eq!(matrix.codomain(), 2);
        assert_eq!(
            matrix.apply(&input).expect("valid product").values(),
            &[50.0, 110.0]
        );
    }

    #[test]
    fn matrix_vector_product_rejects_input_length_mismatch() {
        let matrix = DenseMatrix::try_zeros(Shape::new(2, 3), 0.0).expect("valid matrix");
        let input = Vector::try_from_vec(vec![1.0, 2.0]).expect("valid vector");
        assert!(matches!(
            matrix.apply(&input),
            Err(LinearError::StorageLengthMismatch {
                expected: 3,
                actual: 2
            })
        ));
    }
}
