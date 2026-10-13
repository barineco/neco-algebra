#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod complex_lu;
mod condition;
mod convert;
mod nonsymmetric;
mod refine;

pub use complex_lu::ComplexLu;
pub use condition::condition_number_2;
pub use nonsymmetric::{solve_nonsymmetric_f64, NonsymmetricEigenDecomposition};

use core::fmt;
use faer::dyn_stack::{MemBuffer, MemStack};
use faer::linalg::evd::{
    self_adjoint_evd, self_adjoint_evd_scratch, ComputeEigenvectors, SelfAdjointEvdParams,
};
use faer::{Mat, Spec, Unbind};
use neco_eigensolve::{EigensolveConfig, EigensolveError, EigensolveRequest, EigensolveResult};
use neco_generalized_eigen::{
    ConvergenceStatus, EigenShift, Eigenpair, Eigenspace, GeneralizedEigenError,
    GeneralizedEigenProblem,
};
use neco_linear_dense::DenseMatrix;
use neco_linear_types::{LinearError, Vector};

#[derive(Clone, Debug, PartialEq)]
pub enum EigensolveFaerError {
    InvalidStiffnessMatrix {
        reason: &'static str,
    },
    InvalidMassMatrix {
        reason: &'static str,
    },
    NonPositiveMassMatrix,
    InvalidMatrix {
        operation: &'static str,
        reason: &'static str,
    },
    SingularPivot {
        operation: &'static str,
        index: usize,
        magnitude: f64,
    },
    RightHandSide {
        operation: &'static str,
        expected: usize,
        actual: usize,
    },
    NonFiniteResult {
        operation: &'static str,
    },
    EigenvectorRefinement {
        operation: &'static str,
        index: usize,
        residual: f64,
        kind: RefinementFailureKind,
    },
    ExternalSolver {
        operation: &'static str,
        reason: &'static str,
    },
    Core(EigensolveError),
    Generalized(GeneralizedEigenError),
    Linear(LinearError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefinementFailureKind {
    ResidualExceeded,
    RealEigenvalue,
    LinearlyDependent,
    NearRealAxis,
    SubproblemFailed,
}

impl From<EigensolveError> for EigensolveFaerError {
    fn from(error: EigensolveError) -> Self {
        Self::Core(error)
    }
}

impl From<GeneralizedEigenError> for EigensolveFaerError {
    fn from(error: GeneralizedEigenError) -> Self {
        Self::Generalized(error)
    }
}

impl From<LinearError> for EigensolveFaerError {
    fn from(error: LinearError) -> Self {
        Self::Linear(error)
    }
}

impl fmt::Display for EigensolveFaerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStiffnessMatrix { reason } => {
                write!(formatter, "invalid stiffness matrix: {reason}")
            }
            Self::InvalidMassMatrix { reason } => {
                write!(formatter, "invalid mass matrix: {reason}")
            }
            Self::NonPositiveMassMatrix => {
                write!(formatter, "mass matrix is not positive definite")
            }
            Self::InvalidMatrix { operation, reason } => {
                write!(formatter, "invalid matrix in {operation}: {reason}")
            }
            Self::SingularPivot {
                operation,
                index,
                magnitude,
            } => {
                write!(
                    formatter,
                    "pivot {index} in {operation} is zero or not finite (magnitude {magnitude})"
                )
            }
            Self::RightHandSide {
                operation,
                expected,
                actual,
            } => {
                write!(
                    formatter,
                    "right-hand side in {operation} has {actual} rows, expected {expected}"
                )
            }
            Self::EigenvectorRefinement {
                operation,
                index,
                residual,
                kind,
            } => {
                write!(
                    formatter,
                    "eigenpair {index} in {operation} has residual {residual} ({kind:?})"
                )
            }
            Self::NonFiniteResult { operation } => {
                write!(formatter, "{operation} produced a non-finite result")
            }
            Self::ExternalSolver { operation, reason } => {
                write!(formatter, "external solver failed in {operation}: {reason}")
            }
            Self::Core(error) => error.fmt(formatter),
            Self::Generalized(error) => error.fmt(formatter),
            Self::Linear(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for EigensolveFaerError {}

pub fn solve_symmetric_f64(
    problem: &GeneralizedEigenProblem,
    config: EigensolveConfig,
) -> Result<EigensolveResult, EigensolveFaerError> {
    solve_parts(problem, config, (), "solve_symmetric_f64")
}

pub fn solve_request_symmetric_f64<R>(
    request: EigensolveRequest<R>,
) -> Result<EigensolveResult<R>, EigensolveFaerError> {
    let (problem, config, projection_reference) = request.into_parts();
    solve_parts(
        &problem,
        config,
        projection_reference,
        "solve_request_symmetric_f64",
    )
}

/// Approximate solution to the mass equation, reusing the same Cholesky path
/// as the solver. Serves only as a correction input for an independently
/// certified bound, never as a bound itself.
pub fn approximate_mass_solve(
    mass: &DenseMatrix<f64>,
    rhs: &Vector<f64>,
) -> Result<Vector<f64>, EigensolveFaerError> {
    let dimension = mass.rows();
    if rhs.len() != dimension {
        return Err(EigensolveFaerError::Linear(
            LinearError::StorageLengthMismatch {
                expected: dimension,
                actual: rhs.len(),
            },
        ));
    }
    validate_symmetric_finite(mass, "mass")?;
    let lower = cholesky_lower(mass)?;
    let inverse_lower = inverse_lower_triangular(&lower)?;

    let mut intermediate = Vec::new();
    intermediate
        .try_reserve_exact(dimension)
        .map_err(|_| allocation_error(dimension))?;
    for row in 0..dimension {
        let mut value = 0.0;
        for column in 0..dimension {
            value += inverse_lower[(row, column)] * rhs.values()[column];
        }
        if !value.is_finite() {
            return Err(EigensolveFaerError::ExternalSolver {
                operation: "approximate_mass_solve",
                reason: "mass equation intermediate solution is not finite",
            });
        }
        intermediate.push(value);
    }

    let mut solution = Vec::new();
    solution
        .try_reserve_exact(dimension)
        .map_err(|_| allocation_error(dimension))?;
    for row in 0..dimension {
        let mut value = 0.0;
        for column in 0..dimension {
            value += inverse_lower[(column, row)] * intermediate[column];
        }
        if !value.is_finite() {
            return Err(EigensolveFaerError::ExternalSolver {
                operation: "approximate_mass_solve",
                reason: "mass equation solution is not finite",
            });
        }
        solution.push(value);
    }
    Ok(Vector::try_from_vec(solution)?)
}

/// Transforms the generalized problem into a symmetric standard problem using
/// its positive-definite mass matrix.
fn solve_parts<R>(
    problem: &GeneralizedEigenProblem,
    config: EigensolveConfig,
    projection_reference: R,
    operation: &'static str,
) -> Result<EigensolveResult<R>, EigensolveFaerError> {
    validate_symmetric_finite(problem.stiffness(), "stiffness")?;
    validate_symmetric_finite(problem.mass(), "mass")?;

    let stiffness = to_faer_matrix(problem.stiffness())?;
    let (reduced_stiffness, inverse_lower) = if is_exact_identity(problem.mass())? {
        (stiffness, None)
    } else {
        let lower_mass = cholesky_lower(problem.mass())?;
        let inverse_lower = inverse_lower_triangular(&lower_mass)?;
        let reduced_stiffness = congruence_transform(&inverse_lower, &stiffness, operation)?;
        (reduced_stiffness, Some(inverse_lower))
    };
    let (eigenvalues, eigenvector_values) = decompose_qr(reduced_stiffness.as_ref(), operation)?;
    let dimension = problem.dimension();
    let eigenvectors =
        faer::MatRef::from_column_major_slice(&eigenvector_values, dimension, dimension);
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(dimension)
        .map_err(|_| allocation_error(dimension))?;
    for (column, &eigenvalue) in eigenvalues.iter().enumerate() {
        if !eigenvalue.is_finite() {
            return Err(EigensolveFaerError::ExternalSolver {
                operation,
                reason: "generalized eigenvalue is not finite",
            });
        }
        candidates.push((eigenvalue, column));
    }
    stable_sort_candidates(&mut candidates);

    let selected_modes = config.requested_modes().min(dimension);
    let eigenspaces = build_eigenspaces(
        problem,
        eigenvectors,
        inverse_lower.as_ref(),
        &candidates,
        selected_modes,
        operation,
    )?;
    let returned_modes = eigenspaces
        .iter()
        .map(|eigenspace| eigenspace.basis().len())
        .sum();
    let converged_modes = eigenspaces
        .iter()
        .flat_map(|eigenspace| eigenspace.basis())
        .filter(|pair| {
            pair.residual()
                .meets_tolerance(config.absolute_tolerance(), config.relative_tolerance())
        })
        .count();
    let convergence = if converged_modes == returned_modes {
        ConvergenceStatus::converged(
            1,
            config.requested_modes(),
            returned_modes,
            converged_modes,
            config.absolute_tolerance(),
            config.relative_tolerance(),
        )?
    } else {
        ConvergenceStatus::iteration_limit(
            1,
            config.requested_modes(),
            returned_modes,
            converged_modes,
            config.absolute_tolerance(),
            config.relative_tolerance(),
        )?
    };
    let result = EigensolveResult::from_projected_parts(
        eigenspaces,
        convergence,
        EigenShift::new(neco_complex::Complex::<f64>::zero())?,
        projection_reference,
    )?;
    Ok(result)
}

fn decompose_qr(
    matrix: faer::MatRef<'_, f64>,
    operation: &'static str,
) -> Result<(Vec<f64>, Vec<f64>), EigensolveFaerError> {
    let dimension = matrix.nrows();
    if matrix.ncols() != dimension {
        return Err(LinearError::DimensionMismatch {
            expected_rows: dimension,
            expected_columns: dimension,
            actual_rows: matrix.nrows(),
            actual_columns: matrix.ncols(),
        }
        .into());
    }
    if dimension == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let recursion_threshold = dimension
        .checked_add(1)
        .ok_or(LinearError::CapacityOverflow {
            requested: usize::MAX,
        })?;
    let vector_elements =
        dimension
            .checked_mul(dimension)
            .ok_or(LinearError::CapacityOverflow {
                requested: usize::MAX,
            })?;
    check_f64_capacity(dimension)?;
    check_f64_capacity(vector_elements)?;
    let mut eigenvalues = zeroed_factor_buffer(dimension)?;
    let mut eigenvectors = zeroed_factor_buffer(vector_elements)?;
    let mut params = Spec::<SelfAdjointEvdParams, f64>::default();
    params.config.recursion_threshold = recursion_threshold;
    let parallelism = faer::get_global_parallelism();
    let scratch =
        self_adjoint_evd_scratch::<f64>(dimension, ComputeEigenvectors::Yes, parallelism, params);
    let mut workspace = try_workspace(scratch)?;
    self_adjoint_evd(
        matrix,
        faer::diag::DiagMut::from_slice_mut(&mut eigenvalues),
        Some(faer::MatMut::from_column_major_slice_mut(
            &mut eigenvectors,
            dimension,
            dimension,
        )),
        parallelism,
        MemStack::new(&mut workspace),
        params,
    )
    .map_err(|_| EigensolveFaerError::ExternalSolver {
        operation,
        reason: "symmetric eigendecomposition did not converge",
    })?;
    Ok((eigenvalues, eigenvectors))
}

fn check_f64_capacity(elements: usize) -> Result<(), EigensolveFaerError> {
    let bytes =
        elements
            .checked_mul(core::mem::size_of::<f64>())
            .ok_or(LinearError::CapacityOverflow {
                requested: elements,
            })?;
    if bytes > isize::MAX as usize {
        return Err(LinearError::CapacityOverflow {
            requested: elements,
        }
        .into());
    }
    Ok(())
}

fn zeroed_factor_buffer(elements: usize) -> Result<Vec<f64>, EigensolveFaerError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(elements)
        .map_err(|_| allocation_error(elements))?;
    values.resize(elements, 0.0);
    Ok(values)
}

fn validate_symmetric_finite(
    matrix: &DenseMatrix<f64>,
    matrix_name: &'static str,
) -> Result<(), EigensolveFaerError> {
    let shape = matrix.shape();
    for row in 0..matrix.rows() {
        for column in 0..matrix.columns() {
            let value = *matrix.value(shape.row_index(row)?, shape.column_index(column)?)?;
            if !value.is_finite() {
                return Err(match matrix_name {
                    "stiffness" => EigensolveFaerError::InvalidStiffnessMatrix {
                        reason: "values must be finite",
                    },
                    _ => EigensolveFaerError::InvalidMassMatrix {
                        reason: "values must be finite",
                    },
                });
            }
            let transposed = *matrix.value(shape.row_index(column)?, shape.column_index(row)?)?;
            if value != transposed {
                return Err(match matrix_name {
                    "stiffness" => EigensolveFaerError::InvalidStiffnessMatrix {
                        reason: "matrix must be symmetric",
                    },
                    _ => EigensolveFaerError::InvalidMassMatrix {
                        reason: "matrix must be symmetric",
                    },
                });
            }
        }
    }
    Ok(())
}

fn is_exact_identity(matrix: &DenseMatrix<f64>) -> Result<bool, EigensolveFaerError> {
    let shape = matrix.shape();
    for row in 0..matrix.rows() {
        for column in 0..matrix.columns() {
            let value = *matrix.value(shape.row_index(row)?, shape.column_index(column)?)?;
            let expected: f64 = if row == column { 1.0 } else { 0.0 };
            if value.to_bits() != expected.to_bits() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn to_faer_matrix(matrix: &DenseMatrix<f64>) -> Result<Mat<f64>, EigensolveFaerError> {
    let rows = matrix.rows();
    Ok(Mat::from_fn(
        matrix.rows(),
        matrix.columns(),
        |row, column| matrix.values()[column.unbound() * rows + row.unbound()],
    ))
}

fn cholesky_lower(matrix: &DenseMatrix<f64>) -> Result<Mat<f64>, EigensolveFaerError> {
    let dimension = matrix.rows();
    let shape = matrix.shape();
    let mut lower = Mat::zeros(dimension, dimension);
    for row in 0..dimension {
        for column in 0..=row {
            let mut value = *matrix.value(shape.row_index(row)?, shape.column_index(column)?)?;
            for index in 0..column {
                value -= lower[(row, index)] * lower[(column, index)];
            }
            if row == column {
                if value <= 0.0 || !value.is_finite() {
                    return Err(EigensolveFaerError::NonPositiveMassMatrix);
                }
                lower[(row, column)] = value.sqrt();
            } else {
                lower[(row, column)] = value / lower[(column, column)];
            }
        }
    }
    Ok(lower)
}

fn inverse_lower_triangular(lower: &Mat<f64>) -> Result<Mat<f64>, EigensolveFaerError> {
    let dimension = lower.nrows();
    let mut inverse = Mat::zeros(dimension, dimension);
    for column in 0..dimension {
        for row in 0..dimension {
            let mut value = if row == column { 1.0 } else { 0.0 };
            for index in 0..row {
                value -= lower[(row, index)] * inverse[(index, column)];
            }
            let diagonal = lower[(row, row)];
            if diagonal == 0.0 || !diagonal.is_finite() {
                return Err(EigensolveFaerError::NonPositiveMassMatrix);
            }
            inverse[(row, column)] = value / diagonal;
        }
    }
    Ok(inverse)
}

fn congruence_transform(
    inverse_lower: &Mat<f64>,
    stiffness: &Mat<f64>,
    operation: &'static str,
) -> Result<Mat<f64>, EigensolveFaerError> {
    let dimension = stiffness.nrows();
    let mut left_product = Mat::zeros(dimension, dimension);
    faer::linalg::matmul::matmul(
        left_product.as_mut(),
        faer::Accum::Replace,
        inverse_lower.as_ref(),
        stiffness.as_ref(),
        1.0,
        faer::Par::Seq,
    );
    let mut transformed = Mat::zeros(dimension, dimension);
    faer::linalg::matmul::matmul(
        transformed.as_mut(),
        faer::Accum::Replace,
        left_product.as_ref(),
        inverse_lower.transpose(),
        1.0,
        faer::Par::Seq,
    );
    for row in 0..dimension {
        for column in 0..dimension {
            if !transformed[(row, column)].is_finite() {
                return Err(EigensolveFaerError::ExternalSolver {
                    operation,
                    reason: "reduced stiffness matrix is not finite",
                });
            }
        }
    }
    Ok(transformed)
}

fn build_eigenspaces(
    problem: &GeneralizedEigenProblem,
    eigenvectors: faer::MatRef<'_, f64>,
    inverse_lower: Option<&Mat<f64>>,
    candidates: &[(f64, usize)],
    selected_modes: usize,
    operation: &'static str,
) -> Result<Vec<Eigenspace>, EigensolveFaerError> {
    let mut eigenspaces = Vec::new();
    eigenspaces
        .try_reserve_exact(selected_modes)
        .map_err(|_| allocation_error(selected_modes))?;
    let mut selected = 0;
    while selected < selected_modes {
        let (eigenvalue, _) = candidates[selected];
        let mut basis = Vec::new();
        basis
            .try_reserve_exact(problem.dimension() - selected)
            .map_err(|_| allocation_error(problem.dimension() - selected))?;
        loop {
            let (candidate, column) = candidates[selected];
            if !same_eigenvalue(eigenvalue, candidate) {
                break;
            }
            let vector =
                canonicalized_vector(problem, eigenvectors, inverse_lower, column, operation)?;
            basis.push(Eigenpair::new(problem, eigenvalue, vector)?);
            selected = selected
                .checked_add(1)
                .ok_or(LinearError::CapacityOverflow {
                    requested: usize::MAX,
                })?;
            if selected == candidates.len() {
                break;
            }
        }
        eigenspaces.push(Eigenspace::new(problem, eigenvalue, basis)?);
    }
    Ok(eigenspaces)
}

/// Normalizes a generalized eigenvector in the mass inner product and makes
/// its maximum-magnitude component positive.
fn canonicalized_vector(
    problem: &GeneralizedEigenProblem,
    eigenvectors: faer::MatRef<'_, f64>,
    inverse_lower: Option<&Mat<f64>>,
    column: usize,
    operation: &'static str,
) -> Result<Vector<f64>, EigensolveFaerError> {
    let dimension = problem.dimension();
    let mut values = Vec::new();
    values
        .try_reserve_exact(dimension)
        .map_err(|_| allocation_error(dimension))?;
    if let Some(inverse_lower) = inverse_lower {
        for row in 0..dimension {
            let mut value = 0.0;
            for index in 0..dimension {
                value += inverse_lower[(index, row)] * eigenvectors[(index, column)];
            }
            if !value.is_finite() {
                return Err(EigensolveFaerError::ExternalSolver {
                    operation,
                    reason: "eigenvector is not finite",
                });
            }
            values.push(value);
        }
    } else {
        for row in 0..dimension {
            if !eigenvectors[(row, column)].is_finite() {
                return Err(EigensolveFaerError::ExternalSolver {
                    operation,
                    reason: "eigenvector is not finite",
                });
            }
        }
        for row in 0..dimension {
            let value = eigenvectors[(row, column)];
            values.push(if value == 0.0 { 0.0 } else { value });
        }
    }
    let mass_norm_squared = if inverse_lower.is_some() {
        mass_inner_product(problem.mass(), &values, &values)?
    } else {
        let mut sum = 0.0;
        for value in &values {
            sum += value * value;
        }
        sum
    };
    if !mass_norm_squared.is_finite() || mass_norm_squared <= 0.0 {
        return Err(EigensolveFaerError::NonPositiveMassMatrix);
    }
    let mass_norm = mass_norm_squared.sqrt();
    for value in &mut values {
        *value /= mass_norm;
    }
    let maximum = values
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.abs().total_cmp(&right.abs()))
        .ok_or(EigensolveFaerError::ExternalSolver {
            operation,
            reason: "eigenvector is empty",
        })?;
    if *maximum.1 < 0.0 {
        for value in &mut values {
            *value = -*value;
        }
    }
    Ok(Vector::try_from_vec(values)?)
}

fn mass_inner_product(
    mass: &DenseMatrix<f64>,
    left: &[f64],
    right: &[f64],
) -> Result<f64, EigensolveFaerError> {
    let shape = mass.shape();
    let mut sum = 0.0;
    for (column, right_value) in right.iter().enumerate() {
        for (row, left_value) in left.iter().enumerate() {
            let value = *mass.value(shape.row_index(row)?, shape.column_index(column)?)?;
            sum += left_value * value * right_value;
        }
    }
    Ok(sum)
}

/// Sorts ascending while preserving input order for equal eigenvalues.
fn stable_sort_candidates(candidates: &mut [(f64, usize)]) {
    for sorted_end in 1..candidates.len() {
        let candidate = candidates[sorted_end];
        let mut position = sorted_end;
        while position > 0 && candidates[position - 1].0 > candidate.0 {
            candidates[position] = candidates[position - 1];
            position -= 1;
        }
        candidates[position] = candidate;
    }
}

fn same_eigenvalue(left: f64, right: f64) -> bool {
    left == right
}

fn allocation_error(requested: usize) -> EigensolveFaerError {
    EigensolveFaerError::Linear(LinearError::AllocationFailure { requested })
}

fn try_workspace(request: faer::dyn_stack::StackReq) -> Result<MemBuffer, EigensolveFaerError> {
    let layout = request
        .layout()
        .map_err(|_| LinearError::CapacityOverflow {
            requested: usize::MAX,
        })?;
    MemBuffer::try_new(request).map_err(|_| allocation_error(layout.size()))
}

fn try_filled<T: Clone>(length: usize, value: T) -> Result<Vec<T>, EigensolveFaerError> {
    length
        .checked_mul(core::mem::size_of::<T>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or(LinearError::CapacityOverflow { requested: length })?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| allocation_error(length))?;
    values.resize(length, value);
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::{solve_request_symmetric_f64, solve_symmetric_f64, EigensolveFaerError};
    use neco_eigensolve::{solve_symmetric_f64 as solve_core, EigensolveConfig, EigensolveRequest};
    use neco_generalized_eigen::{EigenProjector, GeneralizedEigenProblem};
    use neco_linear_dense::DenseMatrix;
    use neco_linear_types::{Shape, Vector};

    fn dense(values: Vec<f64>) -> DenseMatrix<f64> {
        DenseMatrix::from_row_major(Shape::new(2, 2), values).expect("matrix")
    }

    fn config() -> EigensolveConfig {
        EigensolveConfig::new(2, 1.0e-10, 1.0e-10, 8).expect("config")
    }

    #[test]
    fn full_mass_diagonal_problem_returns_mass_normalized_eigenpairs() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![8.0, 0.0, 0.0, 27.0]),
            dense(vec![4.0, 0.0, 0.0, 9.0]),
        )
        .expect("problem");
        let result = solve_symmetric_f64(&problem, config()).expect("result");
        assert_eq!(result.eigenspaces().len(), 2);
        assert_eq!(result.eigenspaces()[0].eigenvalue(), 2.0);
        assert_eq!(result.eigenspaces()[1].eigenvalue(), 3.0);
        assert_eq!(
            result.eigenspaces()[0].basis()[0].eigenvector().values(),
            &[0.5, 0.0]
        );
        assert_eq!(
            result.eigenspaces()[1].basis()[0].eigenvector().values(),
            &[0.0, 1.0 / 3.0]
        );
        assert!(result
            .eigenspaces()
            .iter()
            .flat_map(|eigenspace| eigenspace.basis())
            .all(|pair| pair.residual().absolute() <= 1.0e-12));
    }

    #[test]
    fn non_diagonal_mass_problem_has_small_residuals() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![5.0, 1.0, 1.0, 3.0]),
            dense(vec![2.0, 1.0, 1.0, 2.0]),
        )
        .expect("problem");
        let result = solve_symmetric_f64(&problem, config()).expect("result");
        assert!(result
            .eigenspaces()
            .iter()
            .flat_map(|eigenspace| eigenspace.basis())
            .all(|pair| pair.residual().absolute() <= 1.0e-10));
    }

    #[test]
    fn repeated_eigenvalues_return_a_complete_eigenspace() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![2.0, 0.0, 0.0, 2.0]),
            dense(vec![1.0, 0.0, 0.0, 1.0]),
        )
        .expect("problem");
        let requested_one = EigensolveConfig::new(1, 1.0e-10, 1.0e-10, 8).expect("config");
        let result = solve_symmetric_f64(&problem, requested_one).expect("result");
        assert_eq!(result.eigenspaces().len(), 1);
        assert_eq!(result.eigenspaces()[0].basis().len(), 2);
    }

    #[test]
    fn full_mass_projector_is_idempotent() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![8.0, 0.0, 0.0, 27.0]),
            dense(vec![4.0, 0.0, 0.0, 9.0]),
        )
        .expect("problem");
        let result = solve_symmetric_f64(&problem, config()).expect("result");
        let projector = EigenProjector::new(result.eigenspaces()[0].clone());
        let input = Vector::try_from_vec(vec![2.0, -3.0]).expect("input");
        let once = projector.apply(&input).expect("first projection");
        let twice = projector.apply(&once).expect("second projection");
        assert_eq!(once, twice);
    }

    #[test]
    fn identity_mass_matches_core_eigenvalues() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![2.0, 0.0, 0.0, 3.0]),
            dense(vec![1.0, 0.0, 0.0, 1.0]),
        )
        .expect("problem");
        let adapter = solve_symmetric_f64(&problem, config()).expect("adapter");
        let core = solve_core(&problem, config()).expect("core");
        assert_eq!(
            adapter.eigenspaces()[0].eigenvalue(),
            core.eigenspaces()[0].eigenvalue()
        );
        assert_eq!(
            adapter.eigenspaces()[1].eigenvalue(),
            core.eigenspaces()[1].eigenvalue()
        );
    }

    #[test]
    fn request_solver_moves_the_projection_reference_to_the_result() {
        let problem = GeneralizedEigenProblem::from_dense(
            dense(vec![2.0, 0.0, 0.0, 3.0]),
            dense(vec![1.0, 0.0, 0.0, 1.0]),
        )
        .expect("problem");
        let request = EigensolveRequest::new(problem, config(), std::sync::Arc::new("projection"));
        let reference = request.projection_reference().clone();
        let result = solve_request_symmetric_f64(request).expect("result");

        assert!(std::sync::Arc::ptr_eq(
            &reference,
            result.projection_reference()
        ));
    }

    #[test]
    fn invalid_matrices_return_public_failures() {
        let nonsymmetric = GeneralizedEigenProblem::from_dense(
            dense(vec![1.0, 2.0, 0.0, 1.0]),
            dense(vec![1.0, 0.0, 0.0, 1.0]),
        )
        .expect("problem");
        assert!(matches!(
            solve_symmetric_f64(&nonsymmetric, config()),
            Err(EigensolveFaerError::InvalidStiffnessMatrix { .. })
        ));

        let nonsymmetric_mass = GeneralizedEigenProblem::from_dense(
            dense(vec![1.0, 0.0, 0.0, 2.0]),
            dense(vec![1.0, 2.0, 0.0, 1.0]),
        )
        .expect("problem");
        assert!(matches!(
            solve_symmetric_f64(&nonsymmetric_mass, config()),
            Err(EigensolveFaerError::InvalidMassMatrix { .. })
        ));

        let nonfinite_stiffness = GeneralizedEigenProblem::from_dense(
            dense(vec![f64::NAN, 0.0, 0.0, 2.0]),
            dense(vec![1.0, 0.0, 0.0, 1.0]),
        )
        .expect("problem");
        assert!(matches!(
            solve_symmetric_f64(&nonfinite_stiffness, config()),
            Err(EigensolveFaerError::InvalidStiffnessMatrix { .. })
        ));

        let nonpositive_mass = GeneralizedEigenProblem::from_dense(
            dense(vec![1.0, 0.0, 0.0, 2.0]),
            dense(vec![1.0, 0.0, 0.0, -1.0]),
        )
        .expect("problem");
        assert!(matches!(
            solve_symmetric_f64(&nonpositive_mass, config()),
            Err(EigensolveFaerError::NonPositiveMassMatrix)
        ));
    }
}
