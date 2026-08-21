#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;
#[cfg(test)]
extern crate std;

use alloc::vec::Vec;
use core::fmt;
#[cfg(test)]
std::thread_local! {
    static INJECT_NEXT_ALLOCATION_FAILURE: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

use neco_algnum::{AlgnumError, RealAlgebraic};
use neco_bigint::{BigInt, BigintError, Dyadic, ReducedRational};
use neco_expr::{
    project_exact_value_f64, CertifiedScalarProjection, ExactValue, ProjectionPolicy,
    ScalarProjectionError,
};
use neco_formsum::{FormSum, FormSumErrorKind};
use neco_linear_dense::DenseMatrix;
use neco_linear_types::{ColumnIndex, LinearError, RowIndex, Shape, Vector};

#[derive(Debug, Eq, PartialEq)]
pub enum ExactLinearError {
    Linear(LinearError),
    Bigint(BigintError),
    FormSum(FormSumErrorKind),
    Algnum(AlgnumError),
    NonSquareMatrix { rows: usize, columns: usize },
}

impl fmt::Display for ExactLinearError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linear(error) => error.fmt(formatter),
            Self::Bigint(error) => error.fmt(formatter),
            Self::FormSum(error) => error.fmt(formatter),
            Self::Algnum(error) => error.fmt(formatter),
            Self::NonSquareMatrix { rows, columns } => {
                write!(
                    formatter,
                    "determinant requires a square matrix, got {rows}x{columns}"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ExactLinearError {}

impl From<LinearError> for ExactLinearError {
    fn from(error: LinearError) -> Self {
        Self::Linear(error)
    }
}

impl From<BigintError> for ExactLinearError {
    fn from(error: BigintError) -> Self {
        Self::Bigint(error)
    }
}

impl From<FormSumErrorKind> for ExactLinearError {
    fn from(error: FormSumErrorKind) -> Self {
        Self::FormSum(error)
    }
}

impl From<AlgnumError> for ExactLinearError {
    fn from(error: AlgnumError) -> Self {
        Self::Algnum(error)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CertifiedLinearProjectionError {
    Exact(ExactLinearError),
    VectorElement {
        index: usize,
        source: ScalarProjectionError,
    },
    MatrixElement {
        row: usize,
        column: usize,
        source: ScalarProjectionError,
    },
    Linear(LinearError),
    Bigint(BigintError),
}

impl fmt::Display for CertifiedLinearProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact(error) => error.fmt(formatter),
            Self::VectorElement { index, source } => {
                write!(
                    formatter,
                    "vector projection failed at index {index}: {source}"
                )
            }
            Self::MatrixElement {
                row,
                column,
                source,
            } => write!(
                formatter,
                "matrix projection failed at row {row}, column {column}: {source}"
            ),
            Self::Linear(error) => error.fmt(formatter),
            Self::Bigint(error) => error.fmt(formatter),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CertifiedLinearProjectionError {}

impl From<ExactLinearError> for CertifiedLinearProjectionError {
    fn from(error: ExactLinearError) -> Self {
        Self::Exact(error)
    }
}

impl From<LinearError> for CertifiedLinearProjectionError {
    fn from(error: LinearError) -> Self {
        Self::Linear(error)
    }
}

impl From<BigintError> for CertifiedLinearProjectionError {
    fn from(error: BigintError) -> Self {
        Self::Bigint(error)
    }
}

pub trait CertifiedF64Scalar {
    fn project_f64_scalar(
        &self,
        policy: ProjectionPolicy,
    ) -> Result<CertifiedScalarProjection, ScalarProjectionError>;
}

impl CertifiedF64Scalar for ReducedRational {
    fn project_f64_scalar(
        &self,
        policy: ProjectionPolicy,
    ) -> Result<CertifiedScalarProjection, ScalarProjectionError> {
        let value =
            RealAlgebraic::from_reduced_rational(self).map_err(ScalarProjectionError::Algnum)?;
        project_exact_value_f64(&ExactValue::Algebraic(value), policy)
    }
}

impl CertifiedF64Scalar for FormSum {
    fn project_f64_scalar(
        &self,
        policy: ProjectionPolicy,
    ) -> Result<CertifiedScalarProjection, ScalarProjectionError> {
        project_exact_value_f64(
            &ExactValue::FormSum(self.try_clone().map_err(|error| match error {
                FormSumErrorKind::Bigint(error) => ScalarProjectionError::Bigint(error),
                error => ScalarProjectionError::Algnum(AlgnumError::FormSum(error)),
            })?),
            policy,
        )
    }
}

impl CertifiedF64Scalar for RealAlgebraic {
    fn project_f64_scalar(
        &self,
        policy: ProjectionPolicy,
    ) -> Result<CertifiedScalarProjection, ScalarProjectionError> {
        project_exact_value_f64(
            &ExactValue::Algebraic(self.try_clone().map_err(ScalarProjectionError::Algnum)?),
            policy,
        )
    }
}

#[derive(Debug, PartialEq)]
pub struct CertifiedVectorProjection {
    policy: ProjectionPolicy,
    values: Vector<f64>,
    certificates: Vec<CertifiedScalarProjection>,
    absolute_error_bound: Dyadic,
}

impl CertifiedVectorProjection {
    pub const fn policy(&self) -> ProjectionPolicy {
        self.policy
    }

    pub fn values(&self) -> &Vector<f64> {
        &self.values
    }

    pub fn certificates(&self) -> &[CertifiedScalarProjection] {
        &self.certificates
    }

    pub fn absolute_error_bound(&self) -> &Dyadic {
        &self.absolute_error_bound
    }
}

#[derive(Debug, PartialEq)]
pub struct CertifiedMatrixProjection {
    policy: ProjectionPolicy,
    matrix: DenseMatrix<f64>,
    certificates_row_major: Vec<CertifiedScalarProjection>,
    absolute_error_bound: Dyadic,
}

impl CertifiedMatrixProjection {
    pub const fn policy(&self) -> ProjectionPolicy {
        self.policy
    }

    pub fn matrix(&self) -> &DenseMatrix<f64> {
        &self.matrix
    }

    pub fn certificates_row_major(&self) -> &[CertifiedScalarProjection] {
        &self.certificates_row_major
    }

    pub fn absolute_error_bound(&self) -> &Dyadic {
        &self.absolute_error_bound
    }

    pub fn certificate(
        &self,
        row: RowIndex,
        column: ColumnIndex,
    ) -> Result<&CertifiedScalarProjection, CertifiedLinearProjectionError> {
        let shape = self.matrix.shape();
        if row.value() >= shape.rows() {
            return Err(LinearError::IndexOutOfBounds {
                axis: "row",
                index: row.value(),
                bound: shape.rows(),
            }
            .into());
        }
        if column.value() >= shape.columns() {
            return Err(LinearError::IndexOutOfBounds {
                axis: "column",
                index: column.value(),
                bound: shape.columns(),
            }
            .into());
        }
        let index = row
            .value()
            .checked_mul(shape.columns())
            .and_then(|offset| offset.checked_add(column.value()))
            .ok_or(LinearError::CapacityOverflow {
                requested: usize::MAX,
            })?;
        self.certificates_row_major.get(index).ok_or(
            LinearError::InvalidStorage {
                reason: "matrix certificate storage is shorter than its declared shape",
            }
            .into(),
        )
    }
}

pub fn project_vector_f64<T: CertifiedF64Scalar>(
    vector: &Vector<T>,
    policy: ProjectionPolicy,
) -> Result<CertifiedVectorProjection, CertifiedLinearProjectionError> {
    let mut values = Vec::new();
    let mut certificates = Vec::new();
    values
        .try_reserve_exact(vector.len())
        .map_err(|_| LinearError::AllocationFailure {
            requested: vector.len(),
        })?;
    certificates
        .try_reserve_exact(vector.len())
        .map_err(|_| LinearError::AllocationFailure {
            requested: vector.len(),
        })?;
    let mut absolute_error_bound = Dyadic::from_f64_exact(0.0)?;
    for (index, value) in vector.values().iter().enumerate() {
        let certificate = value
            .project_f64_scalar(policy)
            .map_err(|source| CertifiedLinearProjectionError::VectorElement { index, source })?;
        absolute_error_bound = absolute_error_bound.add(certificate.absolute_error())?;
        values.push(certificate.value());
        certificates.push(certificate);
    }
    Ok(CertifiedVectorProjection {
        policy,
        values: Vector::try_from_vec(values)?,
        certificates,
        absolute_error_bound,
    })
}

pub fn project_matrix_f64<T: CertifiedF64Scalar>(
    matrix: &ExactMatrix<T>,
    policy: ProjectionPolicy,
) -> Result<CertifiedMatrixProjection, CertifiedLinearProjectionError> {
    let element_count = matrix.shape().element_count()?;
    let mut values = Vec::new();
    let mut certificates_row_major = Vec::new();
    values
        .try_reserve_exact(element_count)
        .map_err(|_| LinearError::AllocationFailure {
            requested: element_count,
        })?;
    certificates_row_major
        .try_reserve_exact(element_count)
        .map_err(|_| LinearError::AllocationFailure {
            requested: element_count,
        })?;
    let mut absolute_error_bound = Dyadic::from_f64_exact(0.0)?;
    for (index, value) in matrix.values().iter().enumerate() {
        let row = index / matrix.columns();
        let column = index % matrix.columns();
        let certificate = value.project_f64_scalar(policy).map_err(|source| {
            CertifiedLinearProjectionError::MatrixElement {
                row,
                column,
                source,
            }
        })?;
        absolute_error_bound = absolute_error_bound.add(certificate.absolute_error())?;
        values.push(certificate.value());
        certificates_row_major.push(certificate);
    }
    Ok(CertifiedMatrixProjection {
        policy,
        matrix: DenseMatrix::from_row_major(matrix.shape(), values)?,
        certificates_row_major,
        absolute_error_bound,
    })
}

pub trait ExactScalar: Sized {
    fn try_clone(&self) -> Result<Self, ExactLinearError>;

    fn zero() -> Result<Self, ExactLinearError>;

    fn one() -> Result<Self, ExactLinearError>;

    fn is_zero(&self) -> bool;

    fn add(&self, rhs: &Self) -> Result<Self, ExactLinearError>;

    fn sub(&self, rhs: &Self) -> Result<Self, ExactLinearError>;

    fn mul(&self, rhs: &Self) -> Result<Self, ExactLinearError>;

    fn div(&self, rhs: &Self) -> Result<Self, ExactLinearError>;
}

impl ExactScalar for ReducedRational {
    fn try_clone(&self) -> Result<Self, ExactLinearError> {
        Ok(self.try_clone()?)
    }

    fn zero() -> Result<Self, ExactLinearError> {
        Ok(Self::from_bigint(BigInt::zero())?)
    }

    fn one() -> Result<Self, ExactLinearError> {
        Ok(Self::from_bigint(BigInt::one()?)?)
    }

    fn is_zero(&self) -> bool {
        self.is_zero()
    }

    fn add(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.add(rhs)?)
    }

    fn sub(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.sub(rhs)?)
    }

    fn mul(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.mul(rhs)?)
    }

    fn div(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.div(rhs)?)
    }
}

impl ExactScalar for FormSum {
    fn try_clone(&self) -> Result<Self, ExactLinearError> {
        Ok(self.try_clone()?)
    }

    fn zero() -> Result<Self, ExactLinearError> {
        Ok(Self::zero())
    }

    fn one() -> Result<Self, ExactLinearError> {
        Ok(Self::one()?)
    }

    fn is_zero(&self) -> bool {
        self.is_zero()
    }

    fn add(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.add(rhs)?)
    }

    fn sub(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.sub(rhs)?)
    }

    fn mul(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.mul(rhs)?)
    }

    fn div(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.div(rhs)?)
    }
}

impl ExactScalar for RealAlgebraic {
    fn try_clone(&self) -> Result<Self, ExactLinearError> {
        Ok(self.try_clone()?)
    }

    fn zero() -> Result<Self, ExactLinearError> {
        Ok(Self::from_form_sum(&FormSum::zero())?)
    }

    fn one() -> Result<Self, ExactLinearError> {
        Ok(Self::from_form_sum(&FormSum::one()?)?)
    }

    fn is_zero(&self) -> bool {
        self.is_zero()
    }

    fn add(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.add(rhs)?)
    }

    fn sub(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.sub(rhs)?)
    }

    fn mul(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.mul(rhs)?)
    }

    fn div(&self, rhs: &Self) -> Result<Self, ExactLinearError> {
        Ok(self.div(rhs)?)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ExactMatrix<T> {
    shape: Shape,
    values: Vec<T>,
}

impl<T> ExactMatrix<T> {
    pub fn from_row_major(shape: Shape, values: Vec<T>) -> Result<Self, ExactLinearError> {
        let expected = shape.element_count()?;
        if values.len() != expected {
            return Err(LinearError::StorageLengthMismatch {
                expected,
                actual: values.len(),
            }
            .into());
        }
        if values.len() > isize::MAX as usize {
            return Err(LinearError::InvalidStorage {
                reason: "matrix length exceeds the representable allocation limit",
            }
            .into());
        }
        Ok(Self { shape, values })
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

    pub fn value(&self, row: RowIndex, column: ColumnIndex) -> Result<&T, ExactLinearError> {
        let position = self.position(row, column)?;
        self.values.get(position).ok_or_else(|| {
            LinearError::InvalidStorage {
                reason: "matrix storage is shorter than its declared shape",
            }
            .into()
        })
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }

    fn position(&self, row: RowIndex, column: ColumnIndex) -> Result<usize, ExactLinearError> {
        if row.value() >= self.rows() {
            return Err(LinearError::IndexOutOfBounds {
                axis: "row",
                index: row.value(),
                bound: self.rows(),
            }
            .into());
        }
        if column.value() >= self.columns() {
            return Err(LinearError::IndexOutOfBounds {
                axis: "column",
                index: column.value(),
                bound: self.columns(),
            }
            .into());
        }
        row.value()
            .checked_mul(self.columns())
            .and_then(|offset| offset.checked_add(column.value()))
            .ok_or(LinearError::CapacityOverflow {
                requested: usize::MAX,
            })
            .map_err(Into::into)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExactLinearSolution<T> {
    Unique(Vector<T>),
    Affine {
        particular: Vector<T>,
        kernel: Vec<Vector<T>>,
    },
    Inconsistent,
}

impl<T> ExactMatrix<T>
where
    T: ExactScalar,
{
    pub fn determinant(&self) -> Result<T, ExactLinearError> {
        if self.rows() != self.columns() {
            return Err(ExactLinearError::NonSquareMatrix {
                rows: self.rows(),
                columns: self.columns(),
            });
        }
        let elimination = self.eliminate(None)?;
        if elimination.pivots.len() != self.rows() {
            return T::zero();
        }
        let mut determinant = if elimination.swap_count & 1 == 0 {
            T::one()?
        } else {
            T::zero()?.sub(&T::one()?)?
        };
        for pivot in elimination.pivot_values {
            determinant = determinant.mul(&pivot)?;
        }
        Ok(determinant)
    }

    pub fn rank(&self) -> Result<usize, ExactLinearError> {
        Ok(self.eliminate(None)?.pivots.len())
    }

    pub fn kernel_basis(&self) -> Result<Vec<Vector<T>>, ExactLinearError> {
        let elimination = self.eliminate(None)?;
        kernel_basis(&elimination, self.columns())
    }

    pub fn solve(&self, rhs: &Vector<T>) -> Result<ExactLinearSolution<T>, ExactLinearError> {
        if rhs.len() != self.rows() {
            return Err(LinearError::StorageLengthMismatch {
                expected: self.rows(),
                actual: rhs.len(),
            }
            .into());
        }
        let elimination = self.eliminate(Some(rhs))?;
        if elimination.inconsistent {
            return Ok(ExactLinearSolution::Inconsistent);
        }
        let particular = solution_vector(&elimination, self.columns())?;
        if elimination.pivots.len() == self.columns() {
            return Ok(ExactLinearSolution::Unique(particular));
        }
        Ok(ExactLinearSolution::Affine {
            particular,
            kernel: kernel_basis(&elimination, self.columns())?,
        })
    }

    /// Scans columns from left to right and selects the first nonzero row at or
    /// below the pivot row.
    fn eliminate(&self, rhs: Option<&Vector<T>>) -> Result<Elimination<T>, ExactLinearError> {
        let rows = self.rows();
        let columns = self.columns();
        let element_count = self.shape.element_count()?;
        let mut coefficients = try_clone_values(&self.values, element_count)?;
        let mut right_hand_side = match rhs {
            Some(rhs) => Some(try_clone_values(rhs.values(), rows)?),
            None => None,
        };
        let mut pivots = try_vec_capacity(columns)?;
        let mut pivot_values = try_vec_capacity(columns)?;
        let mut pivot_row = 0_usize;
        let mut swap_count = 0_usize;

        for pivot_column in 0..columns {
            let mut selected = None;
            for row in pivot_row..rows {
                if !coefficient(&coefficients, columns, row, pivot_column)?.is_zero() {
                    selected = Some(row);
                    break;
                }
            }
            let Some(selected_row) = selected else {
                continue;
            };
            if selected_row != pivot_row {
                swap_rows(&mut coefficients, columns, selected_row, pivot_row)?;
                if let Some(values) = right_hand_side.as_mut() {
                    values.swap(selected_row, pivot_row);
                }
                swap_count = swap_count
                    .checked_add(1)
                    .ok_or(LinearError::CapacityOverflow {
                        requested: usize::MAX,
                    })?;
            }

            let pivot =
                coefficient(&coefficients, columns, pivot_row, pivot_column)?.try_clone()?;
            pivot_values.push(pivot.try_clone()?);
            normalize_row(
                &mut coefficients,
                right_hand_side.as_mut(),
                columns,
                pivot_row,
                pivot_column,
                &pivot,
            )?;
            for row in 0..rows {
                if row == pivot_row {
                    continue;
                }
                eliminate_row(
                    &mut coefficients,
                    right_hand_side.as_mut(),
                    columns,
                    row,
                    pivot_row,
                    pivot_column,
                )?;
            }
            pivots.push(pivot_column);
            pivot_row = pivot_row
                .checked_add(1)
                .ok_or(LinearError::CapacityOverflow {
                    requested: usize::MAX,
                })?;
            if pivot_row == rows {
                break;
            }
        }

        let inconsistent = match right_hand_side.as_ref() {
            Some(values) => has_inconsistent_row(&coefficients, values, rows, columns)?,
            None => false,
        };
        Ok(Elimination {
            coefficients,
            right_hand_side,
            pivots,
            pivot_values,
            swap_count,
            inconsistent,
        })
    }
}

struct Elimination<T> {
    coefficients: Vec<T>,
    right_hand_side: Option<Vec<T>>,
    pivots: Vec<usize>,
    pivot_values: Vec<T>,
    swap_count: usize,
    inconsistent: bool,
}

fn try_vec_capacity<T>(requested: usize) -> Result<Vec<T>, ExactLinearError> {
    #[cfg(test)]
    if INJECT_NEXT_ALLOCATION_FAILURE.with(|failure| failure.replace(false)) {
        return Err(LinearError::AllocationFailure { requested }.into());
    }

    let mut values = Vec::new();
    values
        .try_reserve_exact(requested)
        .map_err(|_| LinearError::AllocationFailure { requested })?;
    Ok(values)
}

fn try_clone_values<T: ExactScalar>(
    values: &[T],
    requested: usize,
) -> Result<Vec<T>, ExactLinearError> {
    let mut copied = try_vec_capacity(requested)?;
    for value in values {
        copied.push(value.try_clone()?);
    }
    Ok(copied)
}

fn coefficient<T>(
    coefficients: &[T],
    columns: usize,
    row: usize,
    column: usize,
) -> Result<&T, ExactLinearError> {
    let position = row
        .checked_mul(columns)
        .and_then(|offset| offset.checked_add(column))
        .ok_or(LinearError::CapacityOverflow {
            requested: usize::MAX,
        })?;
    coefficients.get(position).ok_or_else(|| {
        LinearError::InvalidStorage {
            reason: "elimination storage is shorter than its declared shape",
        }
        .into()
    })
}

fn coefficient_mut<T>(
    coefficients: &mut [T],
    columns: usize,
    row: usize,
    column: usize,
) -> Result<&mut T, ExactLinearError> {
    let position = row
        .checked_mul(columns)
        .and_then(|offset| offset.checked_add(column))
        .ok_or(LinearError::CapacityOverflow {
            requested: usize::MAX,
        })?;
    coefficients.get_mut(position).ok_or_else(|| {
        LinearError::InvalidStorage {
            reason: "elimination storage is shorter than its declared shape",
        }
        .into()
    })
}

fn swap_rows<T>(
    coefficients: &mut [T],
    columns: usize,
    first: usize,
    second: usize,
) -> Result<(), ExactLinearError> {
    for column in 0..columns {
        let first_position = first
            .checked_mul(columns)
            .and_then(|offset| offset.checked_add(column))
            .ok_or(LinearError::CapacityOverflow {
                requested: usize::MAX,
            })?;
        let second_position = second
            .checked_mul(columns)
            .and_then(|offset| offset.checked_add(column))
            .ok_or(LinearError::CapacityOverflow {
                requested: usize::MAX,
            })?;
        if first_position >= coefficients.len() || second_position >= coefficients.len() {
            return Err(LinearError::InvalidStorage {
                reason: "elimination storage is shorter than its declared shape",
            }
            .into());
        }
        coefficients.swap(first_position, second_position);
    }
    Ok(())
}

fn normalize_row<T: ExactScalar>(
    coefficients: &mut [T],
    right_hand_side: Option<&mut Vec<T>>,
    columns: usize,
    row: usize,
    pivot_column: usize,
    pivot: &T,
) -> Result<(), ExactLinearError> {
    for column in pivot_column..columns {
        let normalized = coefficient(coefficients, columns, row, column)?.div(pivot)?;
        *coefficient_mut(coefficients, columns, row, column)? = normalized;
    }
    if let Some(values) = right_hand_side {
        let value = values.get(row).ok_or(LinearError::InvalidStorage {
            reason: "right-hand-side storage is shorter than its declared shape",
        })?;
        let normalized = value.div(pivot)?;
        *values.get_mut(row).ok_or(LinearError::InvalidStorage {
            reason: "right-hand-side storage is shorter than its declared shape",
        })? = normalized;
    }
    Ok(())
}

fn eliminate_row<T: ExactScalar>(
    coefficients: &mut [T],
    right_hand_side: Option<&mut Vec<T>>,
    columns: usize,
    target_row: usize,
    pivot_row: usize,
    pivot_column: usize,
) -> Result<(), ExactLinearError> {
    let factor = coefficient(coefficients, columns, target_row, pivot_column)?.try_clone()?;
    if factor.is_zero() {
        return Ok(());
    }
    for column in pivot_column..columns {
        let pivot_value = coefficient(coefficients, columns, pivot_row, column)?.try_clone()?;
        let target_value = coefficient(coefficients, columns, target_row, column)?.try_clone()?;
        let value = target_value.sub(&factor.mul(&pivot_value)?)?;
        *coefficient_mut(coefficients, columns, target_row, column)? = value;
    }
    if let Some(values) = right_hand_side {
        let pivot_value = values
            .get(pivot_row)
            .ok_or(LinearError::InvalidStorage {
                reason: "right-hand-side storage is shorter than its declared shape",
            })?
            .try_clone()?;
        let target_value = values
            .get(target_row)
            .ok_or(LinearError::InvalidStorage {
                reason: "right-hand-side storage is shorter than its declared shape",
            })?
            .try_clone()?;
        let value = target_value.sub(&factor.mul(&pivot_value)?)?;
        *values
            .get_mut(target_row)
            .ok_or(LinearError::InvalidStorage {
                reason: "right-hand-side storage is shorter than its declared shape",
            })? = value;
    }
    Ok(())
}

fn has_inconsistent_row<T: ExactScalar>(
    coefficients: &[T],
    right_hand_side: &[T],
    rows: usize,
    columns: usize,
) -> Result<bool, ExactLinearError> {
    if right_hand_side.len() != rows {
        return Err(LinearError::StorageLengthMismatch {
            expected: rows,
            actual: right_hand_side.len(),
        }
        .into());
    }
    for (row, value) in right_hand_side.iter().enumerate() {
        let mut all_zero = true;
        for column in 0..columns {
            if !coefficient(coefficients, columns, row, column)?.is_zero() {
                all_zero = false;
                break;
            }
        }
        if all_zero && !value.is_zero() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn solution_vector<T: ExactScalar>(
    elimination: &Elimination<T>,
    columns: usize,
) -> Result<Vector<T>, ExactLinearError> {
    let right_hand_side =
        elimination
            .right_hand_side
            .as_ref()
            .ok_or(LinearError::InvalidStorage {
                reason: "solution requires a right-hand-side vector",
            })?;
    let mut values = try_vec_capacity(columns)?;
    for _ in 0..columns {
        values.push(T::zero()?);
    }
    for (row, column) in elimination.pivots.iter().copied().enumerate() {
        let value = right_hand_side
            .get(row)
            .ok_or(LinearError::InvalidStorage {
                reason: "right-hand-side storage is shorter than pivot rows",
            })?
            .try_clone()?;
        *values.get_mut(column).ok_or(LinearError::InvalidStorage {
            reason: "solution storage is shorter than its declared dimension",
        })? = value;
    }
    Ok(Vector::try_from_vec(values)?)
}

fn kernel_basis<T: ExactScalar>(
    elimination: &Elimination<T>,
    columns: usize,
) -> Result<Vec<Vector<T>>, ExactLinearError> {
    let mut is_pivot = try_vec_capacity(columns)?;
    is_pivot.resize(columns, false);
    for column in &elimination.pivots {
        *is_pivot
            .get_mut(*column)
            .ok_or(LinearError::InvalidStorage {
                reason: "pivot column exceeds matrix dimension",
            })? = true;
    }
    let free_count =
        columns
            .checked_sub(elimination.pivots.len())
            .ok_or(LinearError::InvalidStorage {
                reason: "pivot count exceeds matrix dimension",
            })?;
    let mut basis = try_vec_capacity(free_count)?;
    for (free_column, pivoted) in is_pivot.iter().copied().enumerate() {
        if pivoted {
            continue;
        }
        let mut values = try_vec_capacity(columns)?;
        for column in 0..columns {
            values.push(if column == free_column {
                T::one()?
            } else {
                T::zero()?
            });
        }
        for (row, pivot_column) in elimination.pivots.iter().copied().enumerate() {
            let factor = coefficient(&elimination.coefficients, columns, row, free_column)?;
            let negated = T::zero()?.sub(factor)?;
            *values
                .get_mut(pivot_column)
                .ok_or(LinearError::InvalidStorage {
                    reason: "kernel vector storage is shorter than matrix dimension",
                })? = negated;
        }
        basis.push(Vector::try_from_vec(values)?);
    }
    Ok(basis)
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{
        project_matrix_f64, project_vector_f64, CertifiedF64Scalar, CertifiedLinearProjectionError,
        ExactLinearSolution, ExactMatrix, ExactScalar,
    };
    use neco_algnum::RealAlgebraic;
    use neco_bigint::{BigInt, Dyadic, ReducedRational};
    use neco_expr::{AbsoluteBits, ProjectionPolicy, ScalarProjectionError};
    use neco_formsum::FormSum;
    use neco_linear_types::{Shape, Vector};

    fn integer(value: i32) -> ReducedRational {
        ReducedRational::from_bigint(BigInt::try_from(value).expect("small integer"))
            .expect("valid rational")
    }

    fn vector(values: &[i32]) -> Vector<ReducedRational> {
        Vector::try_from_vec(values.iter().copied().map(integer).collect()).expect("valid vector")
    }

    fn matrix(rows: usize, columns: usize, values: &[i32]) -> ExactMatrix<ReducedRational> {
        ExactMatrix::from_row_major(
            Shape::new(rows, columns),
            values.iter().copied().map(integer).collect(),
        )
        .expect("valid matrix")
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FailureStage {
        Clone,
        Divide,
        Multiply,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct FailingScalar {
        value: i32,
        failure_stage: FailureStage,
    }

    impl FailingScalar {
        fn new(value: i32, failure_stage: FailureStage) -> Self {
            Self {
                value,
                failure_stage,
            }
        }

        fn failure(reason: &'static str) -> super::ExactLinearError {
            super::ExactLinearError::Linear(neco_linear_types::LinearError::InvalidStorage {
                reason,
            })
        }
    }

    impl ExactScalar for FailingScalar {
        fn try_clone(&self) -> Result<Self, super::ExactLinearError> {
            if self.failure_stage == FailureStage::Clone {
                return Err(Self::failure("test exact scalar clone failure"));
            }
            Ok(*self)
        }

        fn zero() -> Result<Self, super::ExactLinearError> {
            Ok(Self::new(0, FailureStage::Divide))
        }

        fn one() -> Result<Self, super::ExactLinearError> {
            Ok(Self::new(1, FailureStage::Divide))
        }

        fn is_zero(&self) -> bool {
            self.value == 0
        }

        fn add(&self, rhs: &Self) -> Result<Self, super::ExactLinearError> {
            Ok(Self::new(self.value + rhs.value, self.failure_stage))
        }

        fn sub(&self, rhs: &Self) -> Result<Self, super::ExactLinearError> {
            Ok(Self::new(self.value - rhs.value, self.failure_stage))
        }

        fn mul(&self, rhs: &Self) -> Result<Self, super::ExactLinearError> {
            if self.failure_stage == FailureStage::Multiply {
                return Err(Self::failure("test exact scalar multiplication failure"));
            }
            Ok(Self::new(self.value * rhs.value, self.failure_stage))
        }

        fn div(&self, rhs: &Self) -> Result<Self, super::ExactLinearError> {
            if self.failure_stage == FailureStage::Divide {
                return Err(Self::failure("test exact scalar division failure"));
            }
            Ok(Self::new(self.value / rhs.value, self.failure_stage))
        }
    }

    fn failing_matrix(
        rows: usize,
        columns: usize,
        values: &[i32],
        failure_stage: FailureStage,
    ) -> ExactMatrix<FailingScalar> {
        ExactMatrix::from_row_major(
            Shape::new(rows, columns),
            values
                .iter()
                .copied()
                .map(|value| FailingScalar::new(value, failure_stage))
                .collect(),
        )
        .expect("valid matrix")
    }

    fn failing_vector(values: &[i32], failure_stage: FailureStage) -> Vector<FailingScalar> {
        Vector::try_from_vec(
            values
                .iter()
                .copied()
                .map(|value| FailingScalar::new(value, failure_stage))
                .collect(),
        )
        .expect("valid vector")
    }

    impl CertifiedF64Scalar for FailingScalar {
        fn project_f64_scalar(
            &self,
            _policy: ProjectionPolicy,
        ) -> Result<neco_expr::CertifiedScalarProjection, ScalarProjectionError> {
            Err(ScalarProjectionError::FloatOutOfRange)
        }
    }

    fn error_sum(certificates: &[neco_expr::CertifiedScalarProjection]) -> Dyadic {
        certificates
            .iter()
            .try_fold(
                Dyadic::from_f64_exact(0.0).expect("zero dyadic"),
                |sum, certificate| sum.add(certificate.absolute_error()),
            )
            .expect("error sum")
    }

    #[test]
    fn certified_vector_projection_keeps_each_certificate_and_total_error() {
        let policy = ProjectionPolicy::new(AbsoluteBits::new(20));
        let projection = project_vector_f64(&vector(&[1, -2]), policy).expect("projection");

        assert_eq!(projection.policy(), policy);
        assert_eq!(projection.values().values(), &[1.0, -2.0]);
        assert_eq!(projection.certificates().len(), 2);
        assert_eq!(
            projection
                .certificates()
                .iter()
                .map(neco_expr::CertifiedScalarProjection::value)
                .collect::<Vec<_>>(),
            projection.values().values()
        );
        assert_eq!(
            projection.absolute_error_bound(),
            &error_sum(projection.certificates())
        );
    }

    #[test]
    fn certified_matrix_projection_uses_logical_row_major_certificates() {
        let policy = ProjectionPolicy::new(AbsoluteBits::new(20));
        let projection =
            project_matrix_f64(&matrix(2, 2, &[1, 2, 3, 4]), policy).expect("projection");

        assert_eq!(projection.policy(), policy);
        assert_eq!(projection.matrix().shape(), Shape::new(2, 2));
        assert_eq!(projection.certificates_row_major().len(), 4);
        for (row, column, expected) in [(0, 0, 1.0), (0, 1, 2.0), (1, 0, 3.0), (1, 1, 4.0)] {
            let shape = Shape::new(2, 2);
            let row = shape.row_index(row).expect("row index");
            let column = shape.column_index(column).expect("column index");
            assert_eq!(projection.matrix().value(row, column), Ok(&expected));
            assert_eq!(
                projection.certificate(row, column).unwrap().value(),
                expected
            );
        }
        assert_eq!(
            projection.absolute_error_bound(),
            &error_sum(projection.certificates_row_major())
        );
    }

    #[test]
    fn certified_matrix_projection_reports_the_failing_logical_coordinate() {
        let policy = ProjectionPolicy::new(AbsoluteBits::new(20));
        let values = failing_matrix(2, 2, &[1, 2, 3, 4], FailureStage::Divide);

        assert_eq!(
            project_matrix_f64(&values, policy),
            Err(CertifiedLinearProjectionError::MatrixElement {
                row: 0,
                column: 0,
                source: ScalarProjectionError::FloatOutOfRange,
            })
        );
    }

    #[test]
    fn from_row_major_reports_the_storage_length_payload() {
        assert_eq!(
            ExactMatrix::<u8>::from_row_major(Shape::new(2, 2), vec![1, 2, 3]),
            Err(super::ExactLinearError::Linear(
                neco_linear_types::LinearError::StorageLengthMismatch {
                    expected: 4,
                    actual: 3,
                }
            ))
        );
    }

    #[test]
    fn determinant_reports_the_non_square_shape_payload() {
        let values = matrix(2, 3, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(
            values.determinant(),
            Err(super::ExactLinearError::NonSquareMatrix {
                rows: 2,
                columns: 3,
            })
        );
    }

    #[test]
    fn solve_reports_the_right_hand_side_length_payload() {
        let values = matrix(2, 2, &[1, 0, 0, 1]);
        assert_eq!(
            values.solve(&vector(&[1])),
            Err(super::ExactLinearError::Linear(
                neco_linear_types::LinearError::StorageLengthMismatch {
                    expected: 2,
                    actual: 1,
                }
            ))
        );
    }

    #[test]
    fn determinant_preserves_a_pivot_normalization_failure() {
        let values = failing_matrix(1, 1, &[1], FailureStage::Divide);
        assert_eq!(
            values.determinant(),
            Err(super::ExactLinearError::Linear(
                neco_linear_types::LinearError::InvalidStorage {
                    reason: "test exact scalar division failure",
                }
            ))
        );
    }

    #[test]
    fn solve_preserves_a_row_elimination_failure() {
        let values = failing_matrix(2, 1, &[1, 1], FailureStage::Multiply);
        let rhs = failing_vector(&[1, 1], FailureStage::Multiply);
        assert_eq!(
            values.solve(&rhs),
            Err(super::ExactLinearError::Linear(
                neco_linear_types::LinearError::InvalidStorage {
                    reason: "test exact scalar multiplication failure",
                }
            ))
        );
    }

    #[test]
    fn scalar_failure_preserves_a_coefficient_clone_failure() {
        let values = failing_matrix(1, 1, &[1], FailureStage::Clone);
        let rhs = failing_vector(&[1], FailureStage::Clone);
        assert_eq!(
            values.solve(&rhs),
            Err(super::ExactLinearError::Linear(
                neco_linear_types::LinearError::InvalidStorage {
                    reason: "test exact scalar clone failure",
                }
            ))
        );
    }

    #[test]
    fn determinant_and_rank_use_exact_rationals() {
        let values = matrix(2, 2, &[2, 1, 5, 3]);
        assert_eq!(values.determinant().expect("determinant"), integer(1));
        assert_eq!(values.rank().expect("rank"), 2);
    }

    #[test]
    fn kernel_basis_uses_free_columns_in_ascending_order() {
        let values = matrix(2, 3, &[1, 2, 3, 2, 4, 6]);
        let basis = values.kernel_basis().expect("kernel basis");
        assert_eq!(basis.len(), 2);
        assert_eq!(basis[0].values(), &[integer(-2), integer(1), integer(0)]);
        assert_eq!(basis[1].values(), &[integer(-3), integer(0), integer(1)]);
    }

    #[test]
    fn solve_returns_a_unique_solution() {
        let values = matrix(2, 2, &[2, 1, 5, 3]);
        let solution = values.solve(&vector(&[1, 2])).expect("solution");
        assert_eq!(solution, ExactLinearSolution::Unique(vector(&[1, -1])));
    }

    #[test]
    fn solve_returns_an_affine_solution() {
        let values = matrix(1, 2, &[1, 1]);
        let solution = values.solve(&vector(&[3])).expect("solution");
        assert_eq!(
            solution,
            ExactLinearSolution::Affine {
                particular: vector(&[3, 0]),
                kernel: vec![vector(&[-1, 1])],
            }
        );
    }

    #[test]
    fn solve_returns_inconsistent_for_a_contradictory_system() {
        let values = matrix(2, 1, &[1, 1]);
        assert_eq!(
            values.solve(&vector(&[1, 2])).expect("solution"),
            ExactLinearSolution::Inconsistent
        );
    }

    #[test]
    fn pivot_selection_produces_the_same_deterministic_result() {
        let values = matrix(3, 3, &[0, 1, 1, 1, 1, 0, 1, 0, 1]);
        let first = values.solve(&vector(&[3, 3, 2])).expect("solution");
        let second = values.solve(&vector(&[3, 3, 2])).expect("solution");
        assert_eq!(first, ExactLinearSolution::Unique(vector(&[1, 2, 1])));
        assert_eq!(first, second);
    }

    #[test]
    fn form_sum_adapter_solves_an_invertible_one_by_one_matrix() {
        let one = FormSum::one().expect("one");
        let matrix =
            ExactMatrix::from_row_major(Shape::new(1, 1), vec![one]).expect("valid matrix");
        let rhs = Vector::try_from_vec(vec![FormSum::one().expect("one")]).expect("valid vector");
        let solution = matrix.solve(&rhs).expect("solution");
        assert!(
            matches!(solution, ExactLinearSolution::Unique(value) if value.values()[0] == FormSum::one().expect("one"))
        );
    }

    #[test]
    fn real_algebraic_adapter_computes_an_invertible_one_by_one_determinant() {
        let one = RealAlgebraic::one().expect("one");
        let matrix =
            ExactMatrix::from_row_major(Shape::new(1, 1), vec![one]).expect("valid matrix");
        let determinant = matrix.determinant().expect("determinant");
        assert!(!determinant.is_zero());
    }

    #[test]
    fn ncad015_composability_uses_only_the_public_exact_linear_types() {
        fn fit_system<T: ExactScalar>(
            coefficients: ExactMatrix<T>,
            samples: &Vector<T>,
        ) -> Result<ExactLinearSolution<T>, super::ExactLinearError> {
            coefficients.solve(samples)
        }

        assert_eq!(
            fit_system(matrix(2, 2, &[1, 0, 0, 1]), &vector(&[3, 5])),
            Ok(ExactLinearSolution::Unique(vector(&[3, 5])))
        );
    }

    #[test]
    fn solve_reports_allocation_failure_from_try_vec_capacity() {
        super::INJECT_NEXT_ALLOCATION_FAILURE.with(|failure| failure.set(true));
        let values = matrix(1, 1, &[1]);
        let rhs = vector(&[1]);
        assert_eq!(
            values.solve(&rhs),
            Err(super::ExactLinearError::Linear(
                neco_linear_types::LinearError::AllocationFailure { requested: 1 },
            ))
        );
    }
}
