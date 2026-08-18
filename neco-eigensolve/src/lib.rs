#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use neco_complex::Complex;
use neco_generalized_eigen::{
    ConvergenceStatus, EigenShift, Eigenpair, Eigenspace, GeneralizedEigenError,
    GeneralizedEigenProblem,
};
use neco_linear_dense::DenseMatrix;
use neco_linear_types::{LinearError, Vector};
use neco_sparse::CsrMatrix;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EigensolveConfig {
    requested_modes: usize,
    absolute_tolerance: f64,
    relative_tolerance: f64,
    max_iterations: usize,
}

impl EigensolveConfig {
    pub fn new(
        requested_modes: usize,
        absolute_tolerance: f64,
        relative_tolerance: f64,
        max_iterations: usize,
    ) -> Result<Self, EigensolveError> {
        if requested_modes == 0 {
            return Err(EigensolveError::InvalidConfiguration {
                reason: "requested modes must be positive",
            });
        }
        if !is_finite_nonnegative(absolute_tolerance) || !is_finite_nonnegative(relative_tolerance)
        {
            return Err(EigensolveError::InvalidConfiguration {
                reason: "tolerances must be finite and non-negative",
            });
        }
        if max_iterations == 0 {
            return Err(EigensolveError::InvalidConfiguration {
                reason: "maximum iterations must be positive",
            });
        }
        Ok(Self {
            requested_modes,
            absolute_tolerance,
            relative_tolerance,
            max_iterations,
        })
    }

    pub fn requested_modes(&self) -> usize {
        self.requested_modes
    }

    pub fn absolute_tolerance(&self) -> f64 {
        self.absolute_tolerance
    }

    pub fn relative_tolerance(&self) -> f64 {
        self.relative_tolerance
    }

    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EigensolveError {
    Generalized(GeneralizedEigenError),
    InvalidConfiguration { reason: &'static str },
    InvalidResult { reason: &'static str },
    UnsupportedMassMatrix { reason: &'static str },
    InvalidStiffnessMatrix { reason: &'static str },
    Allocation(LinearError),
}

impl From<GeneralizedEigenError> for EigensolveError {
    fn from(error: GeneralizedEigenError) -> Self {
        Self::Generalized(error)
    }
}

impl From<LinearError> for EigensolveError {
    fn from(error: LinearError) -> Self {
        Self::Allocation(error)
    }
}

impl fmt::Display for EigensolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generalized(error) => error.fmt(formatter),
            Self::InvalidConfiguration { reason } => {
                write!(formatter, "invalid configuration: {reason}")
            }
            Self::InvalidResult { reason } => write!(formatter, "invalid solver result: {reason}"),
            Self::UnsupportedMassMatrix { reason } => {
                write!(formatter, "unsupported mass matrix: {reason}")
            }
            Self::InvalidStiffnessMatrix { reason } => {
                write!(formatter, "invalid stiffness matrix: {reason}")
            }
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EigensolveError {}

#[derive(Clone, Debug, PartialEq)]
pub struct EigensolveRequest<R> {
    problem: GeneralizedEigenProblem,
    config: EigensolveConfig,
    projection_reference: R,
}

impl<R> EigensolveRequest<R> {
    pub fn new(
        problem: GeneralizedEigenProblem,
        config: EigensolveConfig,
        projection_reference: R,
    ) -> Self {
        Self {
            problem,
            config,
            projection_reference,
        }
    }

    pub fn problem(&self) -> &GeneralizedEigenProblem {
        &self.problem
    }

    pub fn config(&self) -> EigensolveConfig {
        self.config
    }

    pub fn projection_reference(&self) -> &R {
        &self.projection_reference
    }

    pub fn into_parts(self) -> (GeneralizedEigenProblem, EigensolveConfig, R) {
        (self.problem, self.config, self.projection_reference)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EigensolveResult<R = ()> {
    eigenspaces: Vec<Eigenspace>,
    convergence: ConvergenceStatus,
    shift: EigenShift,
    projection_reference: R,
}

impl EigensolveResult<()> {
    pub fn from_parts(
        eigenspaces: Vec<Eigenspace>,
        convergence: ConvergenceStatus,
        shift: EigenShift,
    ) -> Result<Self, EigensolveError> {
        Self::from_projected_parts(eigenspaces, convergence, shift, ())
    }
}

impl<R> EigensolveResult<R> {
    pub fn from_projected_parts(
        eigenspaces: Vec<Eigenspace>,
        convergence: ConvergenceStatus,
        shift: EigenShift,
        projection_reference: R,
    ) -> Result<Self, EigensolveError> {
        let mut previous = None;
        let mut returned_modes = 0usize;
        for eigenspace in &eigenspaces {
            if let Some(previous_eigenvalue) = previous {
                if eigenspace.eigenvalue() <= previous_eigenvalue {
                    return Err(EigensolveError::InvalidResult {
                        reason: "eigenspaces must have strictly increasing eigenvalues",
                    });
                }
            }
            previous = Some(eigenspace.eigenvalue());
            returned_modes = returned_modes.checked_add(eigenspace.basis().len()).ok_or(
                LinearError::CapacityOverflow {
                    requested: usize::MAX,
                },
            )?;
        }
        let (reported_modes, converged_modes) = match convergence {
            ConvergenceStatus::Converged {
                returned_modes,
                converged_modes,
                ..
            }
            | ConvergenceStatus::IterationLimit {
                returned_modes,
                converged_modes,
                ..
            } => (returned_modes, converged_modes),
            _ => {
                return Err(EigensolveError::InvalidResult {
                    reason: "unsupported convergence status",
                });
            }
        };
        if reported_modes != returned_modes {
            return Err(EigensolveError::InvalidResult {
                reason: "convergence metadata must equal returned eigenspace modes",
            });
        }
        if converged_modes > returned_modes {
            return Err(EigensolveError::InvalidResult {
                reason: "converged mode count exceeds returned mode count",
            });
        }
        Ok(Self {
            eigenspaces,
            convergence,
            shift,
            projection_reference,
        })
    }

    pub fn eigenspaces(&self) -> &[Eigenspace] {
        &self.eigenspaces
    }

    pub fn convergence(&self) -> ConvergenceStatus {
        self.convergence
    }

    pub fn shift(&self) -> EigenShift {
        self.shift
    }

    pub fn projection_reference(&self) -> &R {
        &self.projection_reference
    }

    pub fn into_projection_reference(self) -> R {
        self.projection_reference
    }
}

pub fn solve_symmetric_f64(
    problem: &GeneralizedEigenProblem,
    config: EigensolveConfig,
) -> Result<EigensolveResult, EigensolveError> {
    solve_parts(problem, config, ())
}

pub fn solve_request_symmetric_f64<R>(
    request: EigensolveRequest<R>,
) -> Result<EigensolveResult<R>, EigensolveError> {
    let (problem, config, projection_reference) = request.into_parts();
    solve_parts(&problem, config, projection_reference)
}

fn solve_parts<R>(
    problem: &GeneralizedEigenProblem,
    config: EigensolveConfig,
    projection_reference: R,
) -> Result<EigensolveResult<R>, EigensolveError> {
    validate_stiffness(problem.stiffness())?;
    validate_identity_mass(problem.mass())?;

    let dimension = problem.dimension();
    let selected_modes = config.requested_modes.min(dimension);
    let (mut diagonalized, mut eigenvectors) = jacobi_workspace(problem.stiffness())?;
    let mut iterations = 0;

    loop {
        let (max_off_diagonal, row, column) = largest_off_diagonal(&diagonalized, dimension)?;
        let threshold = convergence_threshold(&diagonalized, dimension, &config)?;
        if max_off_diagonal <= threshold {
            let summary = summarize_eigenspaces(
                problem,
                &diagonalized,
                &mut eigenvectors,
                dimension,
                selected_modes,
                &config,
            )?;
            if summary.converged_modes == summary.returned_modes {
                let convergence = ConvergenceStatus::converged(
                    iterations,
                    config.requested_modes,
                    summary.returned_modes,
                    summary.converged_modes,
                    config.absolute_tolerance,
                    config.relative_tolerance,
                )?;
                return result_with_convergence(summary, convergence, projection_reference);
            }
            if iterations == config.max_iterations {
                let convergence = ConvergenceStatus::iteration_limit(
                    iterations,
                    config.requested_modes,
                    summary.returned_modes,
                    summary.converged_modes,
                    config.absolute_tolerance,
                    config.relative_tolerance,
                )?;
                return result_with_convergence(summary, convergence, projection_reference);
            }
        } else if iterations == config.max_iterations {
            break;
        }

        jacobi_rotate(&mut diagonalized, &mut eigenvectors, dimension, row, column)?;
        iterations = iterations
            .checked_add(1)
            .ok_or(LinearError::CapacityOverflow {
                requested: usize::MAX,
            })?;
    }

    let summary = summarize_eigenspaces(
        problem,
        &diagonalized,
        &mut eigenvectors,
        dimension,
        selected_modes,
        &config,
    )?;
    let convergence = ConvergenceStatus::iteration_limit(
        iterations,
        config.requested_modes,
        summary.returned_modes,
        summary.converged_modes,
        config.absolute_tolerance,
        config.relative_tolerance,
    )?;
    result_with_convergence(summary, convergence, projection_reference)
}

struct EigenspaceSummary {
    eigenspaces: Vec<Eigenspace>,
    returned_modes: usize,
    converged_modes: usize,
}

fn summarize_eigenspaces(
    problem: &GeneralizedEigenProblem,
    diagonalized: &[f64],
    eigenvectors: &mut [f64],
    dimension: usize,
    selected_modes: usize,
    config: &EigensolveConfig,
) -> Result<EigenspaceSummary, EigensolveError> {
    let eigenspaces = build_eigenspaces(
        problem,
        diagonalized,
        eigenvectors,
        dimension,
        selected_modes,
    )?;
    let returned_modes = eigenspaces.iter().map(|space| space.basis().len()).sum();
    let converged_modes = eigenspaces
        .iter()
        .flat_map(|space| space.basis())
        .filter(|pair| residual_converged(pair, config))
        .count();
    Ok(EigenspaceSummary {
        eigenspaces,
        returned_modes,
        converged_modes,
    })
}

fn result_with_convergence<R>(
    summary: EigenspaceSummary,
    convergence: ConvergenceStatus,
    projection_reference: R,
) -> Result<EigensolveResult<R>, EigensolveError> {
    EigensolveResult::from_projected_parts(
        summary.eigenspaces,
        convergence,
        EigenShift::new(Complex::<f64>::zero())?,
        projection_reference,
    )
}

pub fn solve_csr_symmetric_f64(
    stiffness: &CsrMatrix<f64>,
    mass: &CsrMatrix<f64>,
    config: EigensolveConfig,
) -> Result<EigensolveResult, EigensolveError> {
    let problem = GeneralizedEigenProblem::from_csr(stiffness, mass)?;
    solve_symmetric_f64(&problem, config)
}

fn validate_stiffness(matrix: &DenseMatrix<f64>) -> Result<(), EigensolveError> {
    let dimension = matrix.rows();
    for row in 0..dimension {
        for column in 0..dimension {
            let value = matrix_value(matrix, row, column)?;
            if !value.is_finite() {
                return Err(EigensolveError::InvalidStiffnessMatrix {
                    reason: "values must be finite",
                });
            }
            if matrix_value(matrix, column, row)? != value {
                return Err(EigensolveError::InvalidStiffnessMatrix {
                    reason: "matrix must be symmetric",
                });
            }
        }
    }
    Ok(())
}

fn validate_identity_mass(matrix: &DenseMatrix<f64>) -> Result<(), EigensolveError> {
    let dimension = matrix.rows();
    for row in 0..dimension {
        for column in 0..dimension {
            let value = matrix_value(matrix, row, column)?;
            if !value.is_finite() {
                return Err(EigensolveError::UnsupportedMassMatrix {
                    reason: "values must be finite",
                });
            }
            let expected = if row == column { 1.0 } else { 0.0 };
            if value != expected {
                return Err(EigensolveError::UnsupportedMassMatrix {
                    reason: "the first engine requires the exact identity matrix",
                });
            }
        }
    }
    Ok(())
}

fn jacobi_workspace(matrix: &DenseMatrix<f64>) -> Result<(Vec<f64>, Vec<f64>), EigensolveError> {
    let dimension = matrix.rows();
    let length = checked_square(dimension)?;
    let mut diagonalized = try_zeros(length)?;
    let mut eigenvectors = try_zeros(length)?;
    for column in 0..dimension {
        for row in 0..dimension {
            let position = matrix_position(dimension, row, column)?;
            diagonalized[position] = matrix_value(matrix, row, column)?;
            if row == column {
                eigenvectors[position] = 1.0;
            }
        }
    }
    Ok((diagonalized, eigenvectors))
}

/// Scans in row-major order, which fixes the rotation sequence deterministically.
fn largest_off_diagonal(
    matrix: &[f64],
    dimension: usize,
) -> Result<(f64, usize, usize), EigensolveError> {
    if dimension < 2 {
        return Ok((0.0, 0, 0));
    }
    let mut maximum = 0.0;
    let mut maximum_row = 0;
    let mut maximum_column = 1;
    for row in 0..dimension {
        for column in (row + 1)..dimension {
            let value = matrix[matrix_position(dimension, row, column)?].abs();
            if value > maximum {
                maximum = value;
                maximum_row = row;
                maximum_column = column;
            }
        }
    }
    Ok((maximum, maximum_row, maximum_column))
}

fn convergence_threshold(
    matrix: &[f64],
    dimension: usize,
    config: &EigensolveConfig,
) -> Result<f64, EigensolveError> {
    let mut maximum_diagonal: f64 = 0.0;
    for index in 0..dimension {
        maximum_diagonal =
            maximum_diagonal.max(matrix[matrix_position(dimension, index, index)?].abs());
    }
    Ok(config.absolute_tolerance + config.relative_tolerance * maximum_diagonal)
}

fn jacobi_rotate(
    matrix: &mut [f64],
    eigenvectors: &mut [f64],
    dimension: usize,
    row: usize,
    column: usize,
) -> Result<(), EigensolveError> {
    let diagonal_row = matrix[matrix_position(dimension, row, row)?];
    let diagonal_column = matrix[matrix_position(dimension, column, column)?];
    let off_diagonal = matrix[matrix_position(dimension, row, column)?];
    if off_diagonal == 0.0 {
        return Ok(());
    }
    let tau = (diagonal_column - diagonal_row) / (2.0 * off_diagonal);
    let tangent = if tau >= 0.0 {
        1.0 / (tau + square_root(1.0 + tau * tau))
    } else {
        -1.0 / (-tau + square_root(1.0 + tau * tau))
    };
    let cosine = 1.0 / square_root(1.0 + tangent * tangent);
    let sine = tangent * cosine;

    for index in 0..dimension {
        if index != row && index != column {
            let row_position = matrix_position(dimension, index, row)?;
            let column_position = matrix_position(dimension, index, column)?;
            let row_value = matrix[row_position];
            let column_value = matrix[column_position];
            let next_row = cosine * row_value - sine * column_value;
            let next_column = sine * row_value + cosine * column_value;
            matrix[row_position] = next_row;
            matrix[matrix_position(dimension, row, index)?] = next_row;
            matrix[column_position] = next_column;
            matrix[matrix_position(dimension, column, index)?] = next_column;
        }
    }
    matrix[matrix_position(dimension, row, row)?] = cosine * cosine * diagonal_row
        - 2.0 * sine * cosine * off_diagonal
        + sine * sine * diagonal_column;
    matrix[matrix_position(dimension, column, column)?] = sine * sine * diagonal_row
        + 2.0 * sine * cosine * off_diagonal
        + cosine * cosine * diagonal_column;
    matrix[matrix_position(dimension, row, column)?] = 0.0;
    matrix[matrix_position(dimension, column, row)?] = 0.0;

    for index in 0..dimension {
        let row_position = matrix_position(dimension, index, row)?;
        let column_position = matrix_position(dimension, index, column)?;
        let row_value = eigenvectors[row_position];
        let column_value = eigenvectors[column_position];
        eigenvectors[row_position] = cosine * row_value - sine * column_value;
        eigenvectors[column_position] = sine * row_value + cosine * column_value;
    }
    Ok(())
}

fn build_eigenspaces(
    problem: &GeneralizedEigenProblem,
    diagonalized: &[f64],
    eigenvectors: &mut [f64],
    dimension: usize,
    requested_modes: usize,
) -> Result<Vec<Eigenspace>, EigensolveError> {
    let mut order = Vec::new();
    order
        .try_reserve_exact(dimension)
        .map_err(|_| allocation_error(dimension))?;
    for index in 0..dimension {
        order.push(index);
    }
    stable_insertion_sort(&mut order, diagonalized, dimension);

    let mut eigenspaces = Vec::new();
    eigenspaces
        .try_reserve_exact(requested_modes)
        .map_err(|_| allocation_error(requested_modes))?;
    let mut selected = 0;
    while selected < requested_modes {
        let representative_index = order[selected];
        let eigenvalue =
            diagonalized[matrix_position(dimension, representative_index, representative_index)?];
        let mut basis = Vec::new();
        basis
            .try_reserve_exact(dimension - selected)
            .map_err(|_| allocation_error(dimension - selected))?;
        loop {
            let column = order[selected];
            let candidate = diagonalized[matrix_position(dimension, column, column)?];
            if !same_eigenvalue(eigenvalue, candidate) {
                break;
            }
            canonicalize_column(eigenvectors, dimension, column)?;
            let vector = vector_from_column(eigenvectors, dimension, column)?;
            basis.push(Eigenpair::new(problem, eigenvalue, vector)?);
            selected = selected
                .checked_add(1)
                .ok_or(LinearError::CapacityOverflow {
                    requested: usize::MAX,
                })?;
            if selected == dimension {
                break;
            }
        }
        eigenspaces.push(Eigenspace::new(problem, eigenvalue, basis)?);
    }
    Ok(eigenspaces)
}

fn stable_insertion_sort(order: &mut [usize], diagonalized: &[f64], dimension: usize) {
    for sorted_end in 1..order.len() {
        let inserted = order[sorted_end];
        let mut position = sorted_end;
        while position > 0
            && eigenvalue_ordering(order[position - 1], inserted, diagonalized, dimension)
                == Ordering::Greater
        {
            order[position] = order[position - 1];
            position -= 1;
        }
        order[position] = inserted;
    }
}

fn eigenvalue_ordering(
    left: usize,
    right: usize,
    diagonalized: &[f64],
    dimension: usize,
) -> Ordering {
    let left_value = diagonalized[left * dimension + left];
    let right_value = diagonalized[right * dimension + right];
    match left_value.total_cmp(&right_value) {
        Ordering::Equal => left.cmp(&right),
        ordering => ordering,
    }
}

/// Only exactly equal computed eigenvalues share one eigenspace.
fn same_eigenvalue(left: f64, right: f64) -> bool {
    left == right
}

fn residual_converged(pair: &Eigenpair, config: &EigensolveConfig) -> bool {
    let residual = pair.residual();
    if config.absolute_tolerance == 0.0 && config.relative_tolerance == 0.0 {
        residual.absolute() == 0.0
    } else {
        residual.absolute() <= config.absolute_tolerance
            || residual.relative() <= config.relative_tolerance
    }
}

fn canonicalize_column(
    eigenvectors: &mut [f64],
    dimension: usize,
    column: usize,
) -> Result<(), EigensolveError> {
    let mut norm_square = 0.0;
    let mut maximum_absolute = 0.0;
    let mut maximum_index = 0;
    for row in 0..dimension {
        let value = eigenvectors[matrix_position(dimension, row, column)?];
        norm_square += value * value;
        if value.abs() > maximum_absolute {
            maximum_absolute = value.abs();
            maximum_index = row;
        }
    }
    let norm = square_root(norm_square);
    if !norm.is_finite() || norm == 0.0 {
        return Err(EigensolveError::InvalidStiffnessMatrix {
            reason: "Jacobi eigenvector normalization failed",
        });
    }
    let sign = if eigenvectors[matrix_position(dimension, maximum_index, column)?] < 0.0 {
        -1.0
    } else {
        1.0
    };
    for row in 0..dimension {
        let position = matrix_position(dimension, row, column)?;
        eigenvectors[position] = sign * eigenvectors[position] / norm;
    }
    Ok(())
}

fn vector_from_column(
    eigenvectors: &[f64],
    dimension: usize,
    column: usize,
) -> Result<Vector<f64>, EigensolveError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(dimension)
        .map_err(|_| allocation_error(dimension))?;
    for row in 0..dimension {
        values.push(eigenvectors[matrix_position(dimension, row, column)?]);
    }
    Ok(Vector::try_from_vec(values)?)
}

fn matrix_value(
    matrix: &DenseMatrix<f64>,
    row: usize,
    column: usize,
) -> Result<f64, EigensolveError> {
    let shape = matrix.shape();
    Ok(*matrix.value(shape.row_index(row)?, shape.column_index(column)?)?)
}

fn matrix_position(dimension: usize, row: usize, column: usize) -> Result<usize, LinearError> {
    column
        .checked_mul(dimension)
        .and_then(|offset| offset.checked_add(row))
        .ok_or(LinearError::CapacityOverflow {
            requested: usize::MAX,
        })
}

fn checked_square(dimension: usize) -> Result<usize, EigensolveError> {
    dimension
        .checked_mul(dimension)
        .ok_or(LinearError::CapacityOverflow {
            requested: usize::MAX,
        })
        .map_err(EigensolveError::from)
}

fn try_zeros(length: usize) -> Result<Vec<f64>, EigensolveError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| allocation_error(length))?;
    values.resize(length, 0.0);
    Ok(values)
}

fn allocation_error(requested: usize) -> EigensolveError {
    EigensolveError::Allocation(LinearError::AllocationFailure { requested })
}

fn is_finite_nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

/// A bit-exponent estimate with a fixed iteration count keeps this deterministic.
fn square_root(value: f64) -> f64 {
    if value == 0.0 {
        return 0.0;
    }
    let initial_exponent = ((value.to_bits() >> 52) & 0x7ff) as i32 - 1023;
    let mut estimate = f64::from_bits(((initial_exponent / 2 + 1023) as u64) << 52);
    for _ in 0..8 {
        estimate = 0.5 * (estimate + value / estimate);
    }
    estimate
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{
        jacobi_workspace, largest_off_diagonal, solve_csr_symmetric_f64,
        solve_request_symmetric_f64, solve_symmetric_f64, ConvergenceStatus, EigensolveConfig,
        EigensolveError, EigensolveRequest,
    };
    use neco_complex::Complex;
    use neco_generalized_eigen::{EigenResidual, GeneralizedEigenProblem};
    use neco_linear_dense::DenseMatrix;
    use neco_linear_types::{Shape, Vector};
    use neco_sparse::{CooMatrix, CsrMatrix};

    fn dense(values: Vec<f64>) -> DenseMatrix<f64> {
        DenseMatrix::from_row_major(Shape::new(3, 3), values).expect("matrix")
    }

    fn identity() -> DenseMatrix<f64> {
        dense(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
    }

    fn config() -> EigensolveConfig {
        EigensolveConfig::new(3, 1.0e-12, 1.0e-12, 64).expect("config")
    }

    fn diagonal_problem() -> GeneralizedEigenProblem {
        GeneralizedEigenProblem::from_dense(
            dense(vec![9.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 4.0]),
            identity(),
        )
        .expect("problem")
    }

    fn diagonal_csr(values: &[f64], reverse: bool) -> CsrMatrix<f64> {
        let shape = Shape::new(values.len(), values.len());
        let mut coo = CooMatrix::new(shape);
        let indices: Vec<usize> = if reverse {
            (0..values.len()).rev().collect()
        } else {
            (0..values.len()).collect()
        };
        for index in indices {
            coo.push(
                shape.row_index(index).expect("row"),
                shape.column_index(index).expect("column"),
                values[index],
            )
            .expect("entry");
        }
        coo.to_csr().expect("CSR")
    }

    #[test]
    fn diagonal_identity_problem_returns_ascending_eigenvalues_and_small_residuals() {
        let result = solve_symmetric_f64(&diagonal_problem(), config()).expect("result");
        let values: Vec<f64> = result
            .eigenspaces()
            .iter()
            .map(|space| space.eigenvalue())
            .collect();
        assert_eq!(values, vec![1.0, 4.0, 9.0]);
        for space in result.eigenspaces() {
            for pair in space.basis() {
                assert!(pair.residual().absolute() <= 1.0e-12);
                assert!(pair.residual().relative() <= 1.0e-12);
            }
        }
    }

    #[test]
    fn repeated_eigenvalues_form_one_eigenspace() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 5.0]),
            identity(),
        )
        .expect("problem");
        let result = solve_symmetric_f64(&problem, config()).expect("result");
        assert_eq!(result.eigenspaces().len(), 2);
        assert_eq!(result.eigenspaces()[0].basis().len(), 2);
    }

    #[test]
    fn requested_mode_count_returns_the_complete_repeated_eigenspace() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 5.0]),
            identity(),
        )
        .expect("problem");
        let requested_one = EigensolveConfig::new(1, 1.0e-12, 1.0e-12, 64).expect("config");
        let result = solve_symmetric_f64(&problem, requested_one).expect("result");

        assert_eq!(result.eigenspaces().len(), 1);
        assert_eq!(result.eigenspaces()[0].basis().len(), 2);
        assert_eq!(
            result.convergence(),
            ConvergenceStatus::Converged {
                iterations: 0,
                requested_modes: 1,
                returned_modes: 2,
                converged_modes: 2,
                absolute_tolerance: 1.0e-12,
                relative_tolerance: 1.0e-12,
            }
        );
    }

    #[test]
    fn near_diagonal_eigenvalues_remain_separate_eigenspaces() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![1.0, 0.0, 0.0, 0.0, 1.0000005, 0.0, 0.0, 0.0, 3.0]),
            identity(),
        )
        .expect("problem");

        for (absolute_tolerance, relative_tolerance) in [(1.0e-6, 0.0), (0.0, 1.0e-6)] {
            let config = EigensolveConfig::new(3, absolute_tolerance, relative_tolerance, 64)
                .expect("config");
            let result = solve_symmetric_f64(&problem, config).expect("result");
            assert_eq!(result.eigenspaces().len(), 3);
            assert!(result
                .eigenspaces()
                .iter()
                .all(|eigenspace| eigenspace.basis().len() == 1));
        }
    }

    #[test]
    fn csr_input_order_produces_the_same_result() {
        let stiffness = diagonal_csr(&[3.0, 1.0, 2.0], false);
        let stiffness_reversed = diagonal_csr(&[3.0, 1.0, 2.0], true);
        let mass = diagonal_csr(&[1.0, 1.0, 1.0], false);
        let first = solve_csr_symmetric_f64(&stiffness, &mass, config()).expect("first");
        let second = solve_csr_symmetric_f64(&stiffness_reversed, &mass, config()).expect("second");
        assert_eq!(first, second);
    }

    #[test]
    fn repeated_execution_is_deterministic() {
        let first = solve_symmetric_f64(&diagonal_problem(), config()).expect("first");
        let second = solve_symmetric_f64(&diagonal_problem(), config()).expect("second");
        assert_eq!(first, second);
    }

    #[test]
    fn non_identity_mass_matrix_is_rejected() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0]),
            dense(vec![2.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        )
        .expect("problem");
        assert!(matches!(
            solve_symmetric_f64(&problem, config()),
            Err(EigensolveError::UnsupportedMassMatrix { .. })
        ));
    }

    #[test]
    fn nonsymmetric_stiffness_matrix_is_rejected() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![1.0, 2.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
            identity(),
        )
        .expect("problem");
        assert!(matches!(
            solve_symmetric_f64(&problem, config()),
            Err(EigensolveError::InvalidStiffnessMatrix { .. })
        ));
    }

    #[test]
    fn iteration_limit_returns_partial_result() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![2.0, 1.0, 1.0, 1.0, 3.0, 1.0, 1.0, 1.0, 4.0]),
            identity(),
        )
        .expect("problem");
        let limited = EigensolveConfig::new(3, 0.0, 0.0, 1).expect("config");
        let result = solve_symmetric_f64(&problem, limited).expect("partial result");
        assert!(matches!(
            result.convergence(),
            ConvergenceStatus::IterationLimit {
                iterations: 1,
                requested_modes: 3,
                returned_modes: 3,
                converged_modes: 0,
                ..
            }
        ));
        assert!(!result.eigenspaces().is_empty());
    }

    #[test]
    fn residuals_determine_convergence_after_the_matrix_threshold() {
        let absolute_tolerance = 1.0e-6;
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![
                1.0,
                absolute_tolerance,
                absolute_tolerance,
                absolute_tolerance,
                2.0,
                absolute_tolerance,
                absolute_tolerance,
                absolute_tolerance,
                3.0,
            ]),
            identity(),
        )
        .expect("problem");
        let (diagonalized, eigenvectors) =
            jacobi_workspace(problem.stiffness()).expect("workspace");
        let (maximum, _, _) =
            largest_off_diagonal(&diagonalized, problem.dimension()).expect("maximum");
        assert!(maximum <= absolute_tolerance);
        for index in 0..problem.dimension() {
            let eigenvalue = diagonalized[index * problem.dimension() + index];
            let eigenvector = Vector::try_from_vec(
                (0..problem.dimension())
                    .map(|row| eigenvectors[index * problem.dimension() + row])
                    .collect(),
            )
            .expect("eigenvector");
            let residual =
                EigenResidual::from_problem(&problem, eigenvalue, &eigenvector).expect("residual");
            assert!(residual.absolute() > absolute_tolerance);
        }

        let config = EigensolveConfig::new(3, absolute_tolerance, 0.0, 1).expect("config");
        let result = solve_symmetric_f64(&problem, config).expect("result");
        match result.convergence() {
            ConvergenceStatus::Converged {
                returned_modes,
                converged_modes,
                ..
            } => assert_eq!(converged_modes, returned_modes),
            ConvergenceStatus::IterationLimit { .. } => {}
            _ => panic!("unsupported convergence status"),
        }
    }

    #[test]
    fn invalid_configuration_is_rejected() {
        assert!(matches!(
            EigensolveConfig::new(0, 1.0e-12, 1.0e-12, 1),
            Err(EigensolveError::InvalidConfiguration { .. })
        ));
        assert!(matches!(
            EigensolveConfig::new(1, f64::NAN, 1.0e-12, 1),
            Err(EigensolveError::InvalidConfiguration { .. })
        ));
        assert!(matches!(
            EigensolveConfig::new(1, 1.0e-12, 1.0e-12, 0),
            Err(EigensolveError::InvalidConfiguration { .. })
        ));
    }

    #[test]
    fn result_shift_is_complex_f64_zero() {
        let result = solve_symmetric_f64(&diagonal_problem(), config()).expect("result");
        assert_eq!(result.shift().value(), Complex::<f64>::zero());
    }

    #[test]
    fn request_solver_moves_the_projection_reference_to_the_result() {
        let request = EigensolveRequest::new(
            diagonal_problem(),
            config(),
            alloc::sync::Arc::new("spectral projection"),
        );
        let reference = request.projection_reference().clone();
        let result = solve_request_symmetric_f64(request).expect("result");

        assert!(alloc::sync::Arc::ptr_eq(
            &reference,
            result.projection_reference()
        ));
    }
}
