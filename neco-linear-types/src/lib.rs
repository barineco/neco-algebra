#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinearError {
    DimensionMismatch {
        expected_rows: usize,
        expected_columns: usize,
        actual_rows: usize,
        actual_columns: usize,
    },
    IndexOutOfBounds {
        axis: &'static str,
        index: usize,
        bound: usize,
    },
    StorageLengthMismatch {
        expected: usize,
        actual: usize,
    },
    CapacityOverflow {
        requested: usize,
    },
    AllocationFailure {
        requested: usize,
    },
    InvalidStorage {
        reason: &'static str,
    },
}

impl fmt::Display for LinearError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch {
                expected_rows,
                expected_columns,
                actual_rows,
                actual_columns,
            } => write!(
                formatter,
                "dimension mismatch: expected {expected_rows}x{expected_columns}, got {actual_rows}x{actual_columns}"
            ),
            Self::IndexOutOfBounds { axis, index, bound } => {
                write!(formatter, "{axis} index {index} is out of bounds for {bound}")
            }
            Self::StorageLengthMismatch { expected, actual } => {
                write!(formatter, "storage length mismatch: expected {expected}, got {actual}")
            }
            Self::CapacityOverflow { requested } => {
                write!(formatter, "capacity overflow for {requested} elements")
            }
            Self::AllocationFailure { requested } => {
                write!(formatter, "allocation failed for {requested} elements")
            }
            Self::InvalidStorage { reason } => write!(formatter, "invalid storage: {reason}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LinearError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    rows: usize,
    columns: usize,
}

impl Shape {
    pub const fn new(rows: usize, columns: usize) -> Self {
        Self { rows, columns }
    }

    pub const fn rows(self) -> usize {
        self.rows
    }

    pub const fn columns(self) -> usize {
        self.columns
    }

    pub fn element_count(self) -> Result<usize, LinearError> {
        self.rows
            .checked_mul(self.columns)
            .ok_or(LinearError::CapacityOverflow {
                requested: usize::MAX,
            })
    }

    pub fn row_index(self, index: usize) -> Result<RowIndex, LinearError> {
        if index < self.rows {
            Ok(RowIndex(index))
        } else {
            Err(LinearError::IndexOutOfBounds {
                axis: "row",
                index,
                bound: self.rows,
            })
        }
    }

    pub fn column_index(self, index: usize) -> Result<ColumnIndex, LinearError> {
        if index < self.columns {
            Ok(ColumnIndex(index))
        } else {
            Err(LinearError::IndexOutOfBounds {
                axis: "column",
                index,
                bound: self.columns,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowIndex(usize);

impl RowIndex {
    pub const fn value(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnIndex(usize);

impl ColumnIndex {
    pub const fn value(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vector<T> {
    values: Vec<T>,
}

impl<T> Vector<T> {
    pub fn try_from_vec(values: Vec<T>) -> Result<Self, LinearError> {
        if values.len() > isize::MAX as usize {
            return Err(LinearError::InvalidStorage {
                reason: "vector length exceeds the representable allocation limit",
            });
        }
        Ok(Self { values })
    }

    pub fn try_zeros(length: usize, value: T) -> Result<Self, LinearError>
    where
        T: Clone,
    {
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| LinearError::AllocationFailure { requested: length })?;
        values.resize(length, value);
        Self::try_from_vec(values)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn value(&self, index: usize) -> Result<&T, LinearError> {
        self.values.get(index).ok_or(LinearError::IndexOutOfBounds {
            axis: "vector",
            index,
            bound: self.values.len(),
        })
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }

    pub fn into_values(self) -> Vec<T> {
        self.values
    }
}

pub trait LinearOperator<T> {
    fn domain(&self) -> usize;

    fn codomain(&self) -> usize;

    fn apply(&self, input: &Vector<T>) -> Result<Vector<T>, LinearError>;
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{LinearError, LinearOperator, Shape, Vector};

    #[test]
    fn shape_rejects_invalid_indices_and_detects_size_overflow() {
        let shape = Shape::new(2, 3);
        assert_eq!(shape.element_count(), Ok(6));
        assert!(matches!(
            shape.row_index(2),
            Err(LinearError::IndexOutOfBounds { axis: "row", .. })
        ));
        assert!(matches!(
            shape.column_index(3),
            Err(LinearError::IndexOutOfBounds { axis: "column", .. })
        ));
        assert!(matches!(
            Shape::new(usize::MAX, 2).element_count(),
            Err(LinearError::CapacityOverflow { .. })
        ));
    }

    #[test]
    fn vector_preserves_length_and_values() {
        let vector = Vector::try_zeros(3, 7_u8).expect("small allocation succeeds");
        assert_eq!(vector.len(), 3);
        assert_eq!(vector.values(), &[7, 7, 7]);
        assert_eq!(vector.into_values(), vec![7, 7, 7]);
    }

    struct TestOperator;

    impl LinearOperator<i32> for TestOperator {
        fn domain(&self) -> usize {
            2
        }

        fn codomain(&self) -> usize {
            1
        }

        fn apply(&self, input: &Vector<i32>) -> Result<Vector<i32>, LinearError> {
            if input.len() != self.domain() {
                return Err(LinearError::StorageLengthMismatch {
                    expected: self.domain(),
                    actual: input.len(),
                });
            }
            let output = Vector::try_from_vec(vec![*input.value(0)?])?;
            if output.len() != self.codomain() {
                return Err(LinearError::StorageLengthMismatch {
                    expected: self.codomain(),
                    actual: output.len(),
                });
            }
            Ok(output)
        }
    }

    #[test]
    fn operator_trait_checks_input_and_output_storage_lengths() {
        let operator = TestOperator;
        let input = Vector::try_from_vec(vec![1_i32, 2]).expect("valid storage succeeds");
        assert_eq!(operator.domain(), 2);
        assert_eq!(operator.codomain(), 1);
        assert_eq!(operator.apply(&input).expect("valid dimensions").len(), 1);

        let short = Vector::try_from_vec(vec![1_i32]).expect("valid storage succeeds");
        assert!(matches!(
            operator.apply(&short),
            Err(LinearError::StorageLengthMismatch {
                expected: 2,
                actual: 1
            })
        ));
    }
}
