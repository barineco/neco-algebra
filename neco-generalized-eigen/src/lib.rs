#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt;
use neco_complex::Complex;
use neco_linear_dense::DenseMatrix;
use neco_linear_types::{LinearError, LinearOperator, Shape, Vector};
use neco_sparse::CsrMatrix;

const ORTHONORMAL_TOLERANCE: f64 = 1.0e-12;

#[derive(Clone, Debug, PartialEq)]
pub enum GeneralizedEigenError {
    Linear(LinearError),
    NonSquareMatrix {
        rows: usize,
        columns: usize,
    },
    DimensionMismatch {
        stiffness_rows: usize,
        stiffness_columns: usize,
        mass_rows: usize,
        mass_columns: usize,
    },
    InvalidEigenpair {
        reason: &'static str,
    },
    InvalidProjector {
        reason: &'static str,
    },
    InvalidConvergenceStatus {
        reason: &'static str,
    },
}

impl From<LinearError> for GeneralizedEigenError {
    fn from(error: LinearError) -> Self {
        Self::Linear(error)
    }
}

impl fmt::Display for GeneralizedEigenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linear(error) => error.fmt(formatter),
            Self::NonSquareMatrix { rows, columns } => {
                write!(formatter, "matrix shape {rows}x{columns} must be square")
            }
            Self::DimensionMismatch {
                stiffness_rows,
                stiffness_columns,
                mass_rows,
                mass_columns,
            } => write!(
                formatter,
                "stiffness shape {stiffness_rows}x{stiffness_columns} differs from mass shape {mass_rows}x{mass_columns}"
            ),
            Self::InvalidEigenpair { reason } => write!(formatter, "invalid eigenpair: {reason}"),
            Self::InvalidProjector { reason } => write!(formatter, "invalid eigenprojector: {reason}"),
            Self::InvalidConvergenceStatus { reason } => {
                write!(formatter, "invalid convergence status: {reason}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for GeneralizedEigenError {}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneralizedEigenProblem {
    stiffness: DenseMatrix<f64>,
    mass: DenseMatrix<f64>,
}

impl GeneralizedEigenProblem {
    pub fn from_dense(
        stiffness: DenseMatrix<f64>,
        mass: DenseMatrix<f64>,
    ) -> Result<Self, GeneralizedEigenError> {
        validate_problem_shapes(stiffness.shape(), mass.shape())?;
        Ok(Self { stiffness, mass })
    }

    pub fn from_csr(
        stiffness: &CsrMatrix<f64>,
        mass: &CsrMatrix<f64>,
    ) -> Result<Self, GeneralizedEigenError> {
        validate_problem_shapes(stiffness.shape(), mass.shape())?;
        Self::from_dense(csr_to_dense(stiffness)?, csr_to_dense(mass)?)
    }

    pub fn dimension(&self) -> usize {
        self.stiffness.rows()
    }

    pub fn stiffness(&self) -> &DenseMatrix<f64> {
        &self.stiffness
    }

    pub fn mass(&self) -> &DenseMatrix<f64> {
        &self.mass
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EigenResidual {
    absolute: f64,
    relative: f64,
}

impl EigenResidual {
    pub fn new(absolute: f64, relative: f64) -> Result<Self, GeneralizedEigenError> {
        if !is_finite_nonnegative(absolute) || !is_finite_nonnegative(relative) {
            return Err(GeneralizedEigenError::InvalidEigenpair {
                reason: "residual values must be finite and non-negative",
            });
        }
        Ok(Self { absolute, relative })
    }

    pub fn from_problem(
        problem: &GeneralizedEigenProblem,
        eigenvalue: f64,
        vector: &Vector<f64>,
    ) -> Result<Self, GeneralizedEigenError> {
        validate_eigenvalue(eigenvalue)?;
        validate_vector_dimension(problem.dimension(), vector)?;
        if vector.values().iter().any(|value| !value.is_finite()) {
            return Err(GeneralizedEigenError::InvalidEigenpair {
                reason: "eigenvector values must be finite",
            });
        }
        let stiffness_product = problem.stiffness.apply(vector)?;
        let mass_product = problem.mass.apply(vector)?;
        let mut residual_square = 0.0;
        let mut stiffness_square = 0.0;
        let mut mass_square = 0.0;
        for (stiffness_value, mass_value) in
            stiffness_product.values().iter().zip(mass_product.values())
        {
            let residual_value = *stiffness_value - eigenvalue * *mass_value;
            residual_square += residual_value * residual_value;
            stiffness_square += stiffness_value * stiffness_value;
            mass_square += mass_value * mass_value;
        }
        let absolute = square_root(residual_square);
        let scale = square_root(stiffness_square)
            .max(eigenvalue.abs() * square_root(mass_square))
            .max(f64::MIN_POSITIVE);
        Self::new(absolute, absolute / scale)
    }

    pub fn absolute(&self) -> f64 {
        self.absolute
    }

    pub fn relative(&self) -> f64 {
        self.relative
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Eigenpair {
    eigenvalue: f64,
    eigenvector: Vector<f64>,
    residual: EigenResidual,
}

impl Eigenpair {
    pub fn new(
        problem: &GeneralizedEigenProblem,
        eigenvalue: f64,
        eigenvector: Vector<f64>,
    ) -> Result<Self, GeneralizedEigenError> {
        let residual = EigenResidual::from_problem(problem, eigenvalue, &eigenvector)?;
        Ok(Self {
            eigenvalue,
            eigenvector,
            residual,
        })
    }

    pub fn eigenvalue(&self) -> f64 {
        self.eigenvalue
    }

    pub fn eigenvector(&self) -> &Vector<f64> {
        &self.eigenvector
    }

    pub fn residual(&self) -> EigenResidual {
        self.residual
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Eigenspace {
    eigenvalue: f64,
    basis: Vec<Eigenpair>,
    mass: DenseMatrix<f64>,
}

impl Eigenspace {
    pub fn new(
        problem: &GeneralizedEigenProblem,
        eigenvalue: f64,
        basis: Vec<Eigenpair>,
    ) -> Result<Self, GeneralizedEigenError> {
        validate_eigenvalue(eigenvalue)?;
        let first = basis
            .first()
            .ok_or(GeneralizedEigenError::InvalidProjector {
                reason: "an eigenspace requires at least one basis vector",
            })?;
        let dimension = first.eigenvector.len();
        if dimension != problem.dimension() {
            return Err(GeneralizedEigenError::DimensionMismatch {
                stiffness_rows: problem.dimension(),
                stiffness_columns: 1,
                mass_rows: dimension,
                mass_columns: 1,
            });
        }
        for eigenpair in &basis {
            if eigenpair.eigenvalue != eigenvalue {
                return Err(GeneralizedEigenError::InvalidProjector {
                    reason: "basis eigenvalues must equal the eigenspace eigenvalue",
                });
            }
            if eigenpair.eigenvector.len() != dimension {
                return Err(GeneralizedEigenError::InvalidProjector {
                    reason: "basis vector lengths must match",
                });
            }
            if EigenResidual::from_problem(problem, eigenvalue, eigenpair.eigenvector())?
                != eigenpair.residual()
            {
                return Err(GeneralizedEigenError::InvalidProjector {
                    reason: "basis residuals must match the input problem",
                });
            }
            let norm = mass_inner_product(
                problem.mass(),
                eigenpair.eigenvector.values(),
                eigenpair.eigenvector.values(),
            )?;
            if !norm.is_finite() || (norm - 1.0).abs() > ORTHONORMAL_TOLERANCE {
                return Err(GeneralizedEigenError::InvalidProjector {
                    reason: "basis vectors must have unit mass norm",
                });
            }
        }
        for (index, left) in basis.iter().enumerate() {
            for right in basis.iter().skip(index + 1) {
                if mass_inner_product(
                    problem.mass(),
                    left.eigenvector.values(),
                    right.eigenvector.values(),
                )?
                .abs()
                    >= ORTHONORMAL_TOLERANCE
                {
                    return Err(GeneralizedEigenError::InvalidProjector {
                        reason: "basis vectors must be pairwise mass orthogonal",
                    });
                }
            }
        }
        Ok(Self {
            eigenvalue,
            basis,
            mass: problem.mass().clone(),
        })
    }

    pub fn eigenvalue(&self) -> f64 {
        self.eigenvalue
    }

    pub fn dimension(&self) -> usize {
        self.basis[0].eigenvector.len()
    }

    pub fn basis(&self) -> &[Eigenpair] {
        &self.basis
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EigenProjector {
    eigenspace: Eigenspace,
}

impl EigenProjector {
    pub fn new(eigenspace: Eigenspace) -> Self {
        Self { eigenspace }
    }

    pub fn eigenspace(&self) -> &Eigenspace {
        &self.eigenspace
    }

    pub fn apply(&self, input: &Vector<f64>) -> Result<Vector<f64>, GeneralizedEigenError> {
        if input.len() != self.domain() {
            return Err(GeneralizedEigenError::DimensionMismatch {
                stiffness_rows: self.domain(),
                stiffness_columns: 1,
                mass_rows: input.len(),
                mass_columns: 1,
            });
        }
        let mut projected = Vec::new();
        projected
            .try_reserve_exact(self.domain())
            .map_err(|_| LinearError::AllocationFailure {
                requested: self.domain(),
            })?;
        projected.resize(self.domain(), 0.0);
        for eigenpair in self.eigenspace.basis() {
            let coefficient = mass_inner_product(
                &self.eigenspace.mass,
                eigenpair.eigenvector.values(),
                input.values(),
            )?;
            for (output, basis_value) in projected.iter_mut().zip(eigenpair.eigenvector.values()) {
                *output += coefficient * *basis_value;
            }
        }
        Ok(Vector::try_from_vec(projected)?)
    }
}

impl LinearOperator<f64> for EigenProjector {
    fn domain(&self) -> usize {
        self.eigenspace.dimension()
    }

    fn codomain(&self) -> usize {
        self.eigenspace.dimension()
    }

    fn apply(&self, input: &Vector<f64>) -> Result<Vector<f64>, LinearError> {
        EigenProjector::apply(self, input).map_err(|error| match error {
            GeneralizedEigenError::Linear(linear) => linear,
            GeneralizedEigenError::DimensionMismatch { .. } => LinearError::StorageLengthMismatch {
                expected: self.domain(),
                actual: input.len(),
            },
            _ => LinearError::InvalidStorage {
                reason: "validated eigenspace projection failed",
            },
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvergenceStatus {
    kind: ConvergenceKind,
    iterations: usize,
    requested_modes: usize,
    returned_modes: usize,
    converged_modes: usize,
    absolute_tolerance: f64,
    relative_tolerance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ConvergenceKind {
    Converged,
    IterationLimit,
}

impl ConvergenceStatus {
    pub fn converged(
        iterations: usize,
        requested_modes: usize,
        returned_modes: usize,
        converged_modes: usize,
        absolute_tolerance: f64,
        relative_tolerance: f64,
    ) -> Result<Self, GeneralizedEigenError> {
        validate_tolerances(absolute_tolerance, relative_tolerance)?;
        if converged_modes != returned_modes {
            return Err(GeneralizedEigenError::InvalidConvergenceStatus {
                reason: "converged modes must equal returned modes when converged",
            });
        }
        Ok(Self {
            kind: ConvergenceKind::Converged,
            iterations,
            requested_modes,
            returned_modes,
            converged_modes,
            absolute_tolerance,
            relative_tolerance,
        })
    }

    pub fn iteration_limit(
        iterations: usize,
        requested_modes: usize,
        returned_modes: usize,
        converged_modes: usize,
        absolute_tolerance: f64,
        relative_tolerance: f64,
    ) -> Result<Self, GeneralizedEigenError> {
        validate_tolerances(absolute_tolerance, relative_tolerance)?;
        if converged_modes > returned_modes {
            return Err(GeneralizedEigenError::InvalidConvergenceStatus {
                reason: "converged modes must not exceed returned modes",
            });
        }
        Ok(Self {
            kind: ConvergenceKind::IterationLimit,
            iterations,
            requested_modes,
            returned_modes,
            converged_modes,
            absolute_tolerance,
            relative_tolerance,
        })
    }

    pub fn is_converged(&self) -> bool {
        self.kind == ConvergenceKind::Converged
    }

    pub fn iterations(&self) -> usize {
        self.iterations
    }

    pub fn requested_modes(&self) -> usize {
        self.requested_modes
    }

    pub fn returned_modes(&self) -> usize {
        self.returned_modes
    }

    pub fn converged_modes(&self) -> usize {
        self.converged_modes
    }

    pub fn absolute_tolerance(&self) -> f64 {
        self.absolute_tolerance
    }

    pub fn relative_tolerance(&self) -> f64 {
        self.relative_tolerance
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EigenShift {
    value: Complex<f64>,
}

impl EigenShift {
    pub fn new(value: Complex<f64>) -> Result<Self, GeneralizedEigenError> {
        if !value.real().is_finite() || !value.imaginary().is_finite() {
            return Err(GeneralizedEigenError::InvalidEigenpair {
                reason: "eigen shift components must be finite",
            });
        }
        Ok(Self { value })
    }

    pub fn value(&self) -> Complex<f64> {
        self.value
    }

    pub fn real(&self) -> f64 {
        *self.value.real()
    }

    pub fn imaginary(&self) -> f64 {
        *self.value.imaginary()
    }
}

fn validate_problem_shapes(stiffness: Shape, mass: Shape) -> Result<(), GeneralizedEigenError> {
    validate_square(stiffness)?;
    validate_square(mass)?;
    if stiffness != mass {
        return Err(GeneralizedEigenError::DimensionMismatch {
            stiffness_rows: stiffness.rows(),
            stiffness_columns: stiffness.columns(),
            mass_rows: mass.rows(),
            mass_columns: mass.columns(),
        });
    }
    Ok(())
}

fn validate_square(shape: Shape) -> Result<(), GeneralizedEigenError> {
    if shape.rows() != shape.columns() {
        return Err(GeneralizedEigenError::NonSquareMatrix {
            rows: shape.rows(),
            columns: shape.columns(),
        });
    }
    Ok(())
}

fn csr_to_dense(matrix: &CsrMatrix<f64>) -> Result<DenseMatrix<f64>, GeneralizedEigenError> {
    let shape = matrix.shape();
    let length = shape.element_count()?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| LinearError::AllocationFailure { requested: length })?;
    values.resize(length, 0.0);
    for row in 0..shape.rows() {
        let row_index = shape.row_index(row)?;
        for (column, value) in matrix.row(row_index)?.entries() {
            let position = column
                .checked_mul(shape.rows())
                .and_then(|offset| offset.checked_add(row))
                .ok_or(LinearError::CapacityOverflow {
                    requested: usize::MAX,
                })?;
            values[position] = *value;
        }
    }
    Ok(DenseMatrix::from_column_major(shape, values)?)
}

fn validate_eigenvalue(eigenvalue: f64) -> Result<(), GeneralizedEigenError> {
    if eigenvalue.is_finite() {
        Ok(())
    } else {
        Err(GeneralizedEigenError::InvalidEigenpair {
            reason: "eigenvalue must be finite",
        })
    }
}

fn validate_vector_dimension(
    dimension: usize,
    vector: &Vector<f64>,
) -> Result<(), GeneralizedEigenError> {
    if vector.len() == dimension {
        Ok(())
    } else {
        Err(GeneralizedEigenError::DimensionMismatch {
            stiffness_rows: dimension,
            stiffness_columns: 1,
            mass_rows: vector.len(),
            mass_columns: 1,
        })
    }
}

fn validate_tolerances(
    absolute_tolerance: f64,
    relative_tolerance: f64,
) -> Result<(), GeneralizedEigenError> {
    if is_finite_nonnegative(absolute_tolerance) && is_finite_nonnegative(relative_tolerance) {
        Ok(())
    } else {
        Err(GeneralizedEigenError::InvalidEigenpair {
            reason: "tolerances must be finite and non-negative",
        })
    }
}

fn is_finite_nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn mass_inner_product(
    mass: &DenseMatrix<f64>,
    left: &[f64],
    right: &[f64],
) -> Result<f64, GeneralizedEigenError> {
    if left.len() != mass.columns() || right.len() != mass.rows() {
        return Err(GeneralizedEigenError::DimensionMismatch {
            stiffness_rows: mass.rows(),
            stiffness_columns: mass.columns(),
            mass_rows: left.len(),
            mass_columns: right.len(),
        });
    }
    let mut sum = 0.0;
    let shape = mass.shape();
    for (column, right_value) in right.iter().enumerate() {
        for (row, left_value) in left.iter().enumerate() {
            let value = *mass.value(shape.row_index(row)?, shape.column_index(column)?)?;
            sum += left_value * value * right_value;
        }
    }
    Ok(sum)
}

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
        ConvergenceStatus, EigenProjector, EigenResidual, EigenShift, Eigenpair, Eigenspace,
        GeneralizedEigenError, GeneralizedEigenProblem,
    };
    use neco_complex::Complex;
    use neco_linear_dense::DenseMatrix;
    use neco_linear_types::{LinearOperator, Shape, Vector};
    use neco_sparse::CsrMatrix;

    fn dense(values: Vec<f64>) -> DenseMatrix<f64> {
        DenseMatrix::from_row_major(Shape::new(2, 2), values).expect("valid dense matrix")
    }

    fn problem() -> GeneralizedEigenProblem {
        GeneralizedEigenProblem::from_dense(
            dense(vec![2.0, 0.0, 0.0, 3.0]),
            dense(vec![1.0, 0.0, 0.0, 1.0]),
        )
        .expect("valid problem")
    }

    fn vector(values: Vec<f64>) -> Vector<f64> {
        Vector::try_from_vec(values).expect("valid vector")
    }

    fn pair(eigenvalue: f64, values: Vec<f64>) -> Eigenpair {
        Eigenpair::new(&problem(), eigenvalue, vector(values)).expect("valid eigenpair")
    }

    #[test]
    fn dense_problem_constructor_preserves_matrices() {
        let eigenproblem = problem();
        assert_eq!(eigenproblem.dimension(), 2);
        assert_eq!(eigenproblem.stiffness().values(), &[2.0, 0.0, 0.0, 3.0]);
        assert_eq!(eigenproblem.mass().values(), &[1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn dense_and_csr_shape_mismatches_are_rejected() {
        let rectangular = DenseMatrix::try_zeros(Shape::new(2, 3), 0.0).expect("matrix");
        let square = DenseMatrix::try_zeros(Shape::new(2, 2), 0.0).expect("matrix");
        assert!(matches!(
            GeneralizedEigenProblem::from_dense(rectangular, square),
            Err(GeneralizedEigenError::NonSquareMatrix { .. })
        ));
        let stiffness =
            CsrMatrix::from_parts(Shape::new(2, 2), vec![0, 1, 2], vec![0, 1], vec![1.0, 1.0])
                .expect("CSR matrix");
        let mass = CsrMatrix::from_parts(
            Shape::new(3, 3),
            vec![0, 1, 2, 3],
            vec![0, 1, 2],
            vec![1.0, 1.0, 1.0],
        )
        .expect("CSR matrix");
        assert!(matches!(
            GeneralizedEigenProblem::from_csr(&stiffness, &mass),
            Err(GeneralizedEigenError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn csr_conversion_preserves_column_major_values() {
        let stiffness = CsrMatrix::from_parts(
            Shape::new(2, 2),
            vec![0, 2, 3],
            vec![0, 1, 1],
            vec![1.0, 2.0, 3.0],
        )
        .expect("CSR matrix");
        let mass =
            CsrMatrix::from_parts(Shape::new(2, 2), vec![0, 1, 2], vec![0, 1], vec![1.0, 1.0])
                .expect("CSR matrix");
        let eigenproblem =
            GeneralizedEigenProblem::from_csr(&stiffness, &mass).expect("valid problem");
        assert_eq!(eigenproblem.stiffness().values(), &[1.0, 0.0, 2.0, 3.0]);
    }

    #[test]
    fn diagonal_problem_has_zero_eigen_residual() {
        let residual = EigenResidual::from_problem(&problem(), 2.0, &vector(vec![1.0, 0.0]))
            .expect("residual");
        assert_eq!(residual.absolute(), 0.0);
        assert_eq!(residual.relative(), 0.0);
    }

    #[test]
    fn eigen_residual_rejects_nonfinite_vector_before_arithmetic() {
        assert!(matches!(
            EigenResidual::from_problem(&problem(), 2.0, &vector(vec![f64::NAN, 0.0])),
            Err(GeneralizedEigenError::InvalidEigenpair {
                reason: "eigenvector values must be finite"
            })
        ));
    }

    #[test]
    fn eigenpair_computes_positive_residual_for_non_eigenvector() {
        let eigenpair = Eigenpair::new(&problem(), 2.0, vector(vec![0.0, 1.0])).expect("eigenpair");
        assert!(eigenpair.residual().absolute() > 0.0);
    }

    #[test]
    fn eigenpair_rejects_vector_length_mismatch() {
        assert!(matches!(
            Eigenpair::new(&problem(), 2.0, vector(vec![1.0])),
            Err(GeneralizedEigenError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn two_vector_eigenspace_has_an_idempotent_projector() {
        let eigenproblem = problem();
        let space = Eigenspace::new(
            &eigenproblem,
            2.0,
            vec![pair(2.0, vec![1.0, 0.0]), pair(2.0, vec![0.0, 1.0])],
        )
        .expect("orthonormal basis");
        let projector = EigenProjector::new(space);
        let input = vector(vec![4.0, -3.0]);
        let once = projector.apply(&input).expect("projection");
        let twice = projector.apply(&once).expect("projection");
        assert_eq!(once, twice);
        assert_eq!(
            LinearOperator::apply(&projector, &input).expect("operator"),
            once
        );
    }

    #[test]
    fn mass_orthonormal_basis_has_an_idempotent_projector() {
        let eigenproblem = GeneralizedEigenProblem::from_dense(
            dense(vec![2.0, 0.0, 0.0, 3.0]),
            dense(vec![4.0, 0.0, 0.0, 9.0]),
        )
        .expect("valid problem");
        let space = Eigenspace::new(
            &eigenproblem,
            0.5,
            vec![Eigenpair::new(&eigenproblem, 0.5, vector(vec![0.5, 0.0])).expect("eigenpair")],
        )
        .expect("mass orthonormal basis");
        let projector = EigenProjector::new(space);
        let input = vector(vec![8.0, -3.0]);
        assert_eq!(
            projector.apply(&input).expect("projection").values(),
            &[8.0, 0.0]
        );
    }

    #[test]
    fn non_orthonormal_basis_and_projector_input_mismatch_are_rejected() {
        let eigenproblem = problem();
        assert!(matches!(
            Eigenspace::new(
                &eigenproblem,
                2.0,
                vec![pair(2.0, vec![1.0, 0.0]), pair(2.0, vec![1.0, 0.0])]
            ),
            Err(GeneralizedEigenError::InvalidProjector { .. })
        ));
        let projector = EigenProjector::new(
            Eigenspace::new(&eigenproblem, 2.0, vec![pair(2.0, vec![1.0, 0.0])]).expect("basis"),
        );
        assert!(matches!(
            projector.apply(&vector(vec![1.0])),
            Err(GeneralizedEigenError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn eigenspace_rejects_an_eigenpair_from_a_different_problem() {
        let problem_a = GeneralizedEigenProblem::from_dense(
            dense(vec![1.0, 0.0, 0.0, 2.0]),
            dense(vec![1.0, 0.0, 0.0, 1.0]),
        )
        .expect("problem A");
        let problem_b = GeneralizedEigenProblem::from_dense(
            dense(vec![1.0, 0.0, 0.0, 3.0]),
            dense(vec![1.0, 0.0, 0.0, 1.0]),
        )
        .expect("problem B");
        let pair = Eigenpair::new(&problem_b, 3.0, vector(vec![0.0, 1.0])).expect("pair");
        assert!(matches!(
            Eigenspace::new(&problem_a, 3.0, vec![pair]),
            Err(GeneralizedEigenError::InvalidProjector { .. })
        ));
    }

    #[test]
    fn convergence_status_validates_tolerances_and_mode_counts() {
        assert!(matches!(
            ConvergenceStatus::converged(4, 2, 2, 2, -1.0, 1.0e-8),
            Err(GeneralizedEigenError::InvalidEigenpair { .. })
        ));
        let converged = ConvergenceStatus::converged(4, 1, 2, 2, 1.0e-8, 2.0e-8).expect("status");
        assert!(converged.is_converged());
        assert_eq!(converged.iterations(), 4);
        assert_eq!(converged.requested_modes(), 1);
        assert_eq!(converged.returned_modes(), 2);
        assert_eq!(converged.converged_modes(), 2);
        assert_eq!(converged.absolute_tolerance(), 1.0e-8);
        assert_eq!(converged.relative_tolerance(), 2.0e-8);
        assert!(matches!(
            ConvergenceStatus::converged(4, 2, 2, 1, 1.0e-8, 1.0e-8),
            Err(GeneralizedEigenError::InvalidConvergenceStatus { .. })
        ));
        let limited =
            ConvergenceStatus::iteration_limit(4, 2, 2, 1, 1.0e-8, 1.0e-8).expect("status");
        assert!(!limited.is_converged());
        assert_eq!(limited.returned_modes(), 2);
        assert_eq!(limited.converged_modes(), 1);
        assert!(matches!(
            ConvergenceStatus::iteration_limit(4, 2, 1, 2, 1.0e-8, 1.0e-8),
            Err(GeneralizedEigenError::InvalidConvergenceStatus { .. })
        ));
    }

    #[test]
    fn complex_shift_uses_complex_f64_components() {
        let shift = EigenShift::new(Complex::new(1.5, -0.25)).expect("finite shift");
        assert_eq!(shift.real(), 1.5);
        assert_eq!(shift.imaginary(), -0.25);
        assert_eq!(shift.value(), Complex::new(1.5, -0.25));
    }
}
