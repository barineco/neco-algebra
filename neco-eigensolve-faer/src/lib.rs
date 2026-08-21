#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use core::fmt;
use faer::linalg::solvers::SelfAdjointEigen;
use faer::{Mat, Side, Unbind};
use neco_eigensolve::{EigensolveConfig, EigensolveError, EigensolveRequest, EigensolveResult};
use neco_generalized_eigen::{
    ConvergenceStatus, EigenShift, Eigenpair, Eigenspace, GeneralizedEigenError,
    GeneralizedEigenProblem,
};
use neco_linear_dense::DenseMatrix;
use neco_linear_types::{LinearError, Vector};

#[derive(Clone, Debug, PartialEq)]
pub enum EigensolveFaerError {
    InvalidStiffnessMatrix { reason: &'static str },
    InvalidMassMatrix { reason: &'static str },
    NonPositiveMassMatrix,
    ExternalSolver { reason: &'static str },
    Core(EigensolveError),
    Generalized(GeneralizedEigenError),
    Linear(LinearError),
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
            Self::ExternalSolver { reason } => {
                write!(formatter, "external solver failed: {reason}")
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
    solve_parts(problem, config, ())
}

pub fn solve_request_symmetric_f64<R>(
    request: EigensolveRequest<R>,
) -> Result<EigensolveResult<R>, EigensolveFaerError> {
    let (problem, config, projection_reference) = request.into_parts();
    solve_parts(&problem, config, projection_reference)
}

/// Transforms the generalized problem into a symmetric standard problem using
/// its positive-definite mass matrix.
fn solve_parts<R>(
    problem: &GeneralizedEigenProblem,
    config: EigensolveConfig,
    projection_reference: R,
) -> Result<EigensolveResult<R>, EigensolveFaerError> {
    validate_symmetric_finite(problem.stiffness(), "stiffness")?;
    validate_symmetric_finite(problem.mass(), "mass")?;

    let stiffness = to_faer_matrix(problem.stiffness())?;
    let lower_mass = cholesky_lower(problem.mass())?;
    let inverse_lower = inverse_lower_triangular(&lower_mass)?;
    let reduced_stiffness = congruence_transform(&inverse_lower, &stiffness)?;
    let decomposition =
        SelfAdjointEigen::new(reduced_stiffness.as_ref(), Side::Lower).map_err(|_| {
            EigensolveFaerError::ExternalSolver {
                reason: "symmetric eigendecomposition did not converge",
            }
        })?;

    let dimension = problem.dimension();
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(dimension)
        .map_err(|_| allocation_error(dimension))?;
    for column in 0..dimension {
        let eigenvalue = decomposition.S()[column];
        if !eigenvalue.is_finite() {
            return Err(EigensolveFaerError::ExternalSolver {
                reason: "generalized eigenvalue is not finite",
            });
        }
        candidates.push((eigenvalue, column));
    }
    stable_sort_candidates(&mut candidates);

    let selected_modes = config.requested_modes().min(dimension);
    let eigenspaces = build_eigenspaces(
        problem,
        decomposition.U(),
        &inverse_lower,
        &candidates,
        selected_modes,
    )?;
    let returned_modes = eigenspaces
        .iter()
        .map(|eigenspace| eigenspace.basis().len())
        .sum();
    let converged_modes = eigenspaces
        .iter()
        .flat_map(|eigenspace| eigenspace.basis())
        .filter(|pair| residual_converged(pair, config))
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
    Ok(EigensolveResult::from_projected_parts(
        eigenspaces,
        convergence,
        EigenShift::new(neco_complex::Complex::<f64>::zero())?,
        projection_reference,
    )?)
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
) -> Result<Mat<f64>, EigensolveFaerError> {
    let dimension = stiffness.nrows();
    let mut transformed = Mat::zeros(dimension, dimension);
    for row in 0..dimension {
        for column in 0..dimension {
            let mut value = 0.0;
            for left in 0..dimension {
                for right in 0..dimension {
                    let right_product = stiffness[(left, right)] * inverse_lower[(column, right)];
                    value += inverse_lower[(row, left)] * right_product;
                }
            }
            if !value.is_finite() {
                return Err(EigensolveFaerError::ExternalSolver {
                    reason: "reduced stiffness matrix is not finite",
                });
            }
            transformed[(row, column)] = value;
        }
    }
    Ok(transformed)
}

fn build_eigenspaces(
    problem: &GeneralizedEigenProblem,
    eigenvectors: faer::MatRef<'_, f64>,
    inverse_lower: &Mat<f64>,
    candidates: &[(f64, usize)],
    selected_modes: usize,
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
            let vector = canonicalized_vector(problem, eigenvectors, inverse_lower, column)?;
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
    inverse_lower: &Mat<f64>,
    column: usize,
) -> Result<Vector<f64>, EigensolveFaerError> {
    let dimension = problem.dimension();
    let mut values = Vec::new();
    values
        .try_reserve_exact(dimension)
        .map_err(|_| allocation_error(dimension))?;
    for row in 0..dimension {
        let mut value = 0.0;
        for index in 0..dimension {
            value += inverse_lower[(index, row)] * eigenvectors[(index, column)];
        }
        if !value.is_finite() {
            return Err(EigensolveFaerError::ExternalSolver {
                reason: "eigenvector is not finite",
            });
        }
        values.push(value);
    }
    let mass_norm_squared = mass_inner_product(problem.mass(), &values, &values)?;
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

fn residual_converged(pair: &Eigenpair, config: EigensolveConfig) -> bool {
    let residual = pair.residual();
    if config.absolute_tolerance() == 0.0 && config.relative_tolerance() == 0.0 {
        residual.absolute() == 0.0
    } else {
        residual.absolute() <= config.absolute_tolerance()
            || residual.relative() <= config.relative_tolerance()
    }
}

fn allocation_error(requested: usize) -> EigensolveFaerError {
    EigensolveFaerError::Linear(LinearError::AllocationFailure { requested })
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
