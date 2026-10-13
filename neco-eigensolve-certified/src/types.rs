use core::fmt;
use neco_bigint::{BigintError, Dyadic};
use neco_eigensolve::EigensolveError;
use neco_eigensolve_faer::EigensolveFaerError;
use neco_generalized_eigen::{ConvergenceStatus, EigenResidual, Eigenpair};
use neco_linear_exact::CertifiedGeneralizedEigenInput;
use neco_linear_types::{LinearError, Vector};

/// The lower-layer solver failure behind a certification failure.
#[derive(Clone, Debug, PartialEq)]
pub enum SolverFailure {
    IdentityMass(EigensolveError),
    PositiveDefiniteMass(EigensolveFaerError),
}

impl fmt::Display for SolverFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityMass(error) => error.fmt(formatter),
            Self::PositiveDefiniteMass(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SolverFailure {}

/// An eigenpair whose residual did not satisfy the configured tolerances.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NonConvergentEigenpair {
    pub(crate) eigenvalue: f64,
    pub(crate) residual: EigenResidual,
}

impl NonConvergentEigenpair {
    pub fn eigenvalue(&self) -> f64 {
        self.eigenvalue
    }

    pub fn residual(&self) -> EigenResidual {
        self.residual
    }
}

/// Why an eigenpair could not be certified.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UndeterminableReason {
    MassLowerBoundUnavailable,
    ProjectionPerturbationTooLarge,
    NegativeEigenvalue,
    EnclosureLowerBoundBelowZero { lower_bound: f64 },
    NonFiniteFrequencyBound,
}

impl fmt::Display for UndeterminableReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MassLowerBoundUnavailable => {
                write!(formatter, "no certified positive mass lower bound")
            }
            Self::ProjectionPerturbationTooLarge => {
                write!(formatter, "projection error exceeds the mass lower bound")
            }
            Self::NegativeEigenvalue => write!(formatter, "the eigenvalue is negative"),
            Self::NonFiniteFrequencyBound => {
                write!(formatter, "a frequency bound rounded to infinity or NaN")
            }
            Self::EnclosureLowerBoundBelowZero { lower_bound } => {
                write!(formatter, "enclosure lower bound {lower_bound} is negative")
            }
        }
    }
}

/// One eigenpair that could not be certified, with evidence for the reason.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UndeterminableEigenpair {
    pub(crate) eigenvalue: f64,
    pub(crate) enclosure_radius: Option<f64>,
    pub(crate) reason: UndeterminableReason,
}

impl UndeterminableEigenpair {
    pub fn eigenvalue(&self) -> f64 {
        self.eigenvalue
    }

    pub fn enclosure_radius(&self) -> Option<f64> {
        self.enclosure_radius
    }

    pub fn reason(&self) -> UndeterminableReason {
        self.reason
    }
}

/// A point estimate with a safe containment interval. See the crate
/// documentation for the existence-only guarantee, the symmetry
/// precondition, and the frequency unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CertifiedFrequency {
    pub(crate) value: f64,
    pub(crate) lower_bound: f64,
    pub(crate) upper_bound: f64,
}

impl CertifiedFrequency {
    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn lower_bound(&self) -> f64 {
        self.lower_bound
    }

    pub fn upper_bound(&self) -> f64 {
        self.upper_bound
    }
}

/// An eigenpair with a certified forward-error bound and frequency. See the
/// crate documentation for the existence-only guarantee this provides.
#[derive(Debug, PartialEq)]
pub struct CertifiedEigenpair {
    pub(crate) pair: Eigenpair,
    pub(crate) computational_bound: Dyadic,
    pub(crate) projection_bound: Dyadic,
    pub(crate) enclosure_radius: Dyadic,
    pub(crate) frequency: CertifiedFrequency,
}

impl CertifiedEigenpair {
    pub fn pair(&self) -> &Eigenpair {
        &self.pair
    }

    pub fn eigenvalue(&self) -> f64 {
        self.pair.eigenvalue()
    }

    pub fn eigenvector(&self) -> &Vector<f64> {
        self.pair.eigenvector()
    }

    pub fn residual(&self) -> EigenResidual {
        self.pair.residual()
    }

    pub fn computational_bound(&self) -> &Dyadic {
        &self.computational_bound
    }

    pub fn projection_bound(&self) -> &Dyadic {
        &self.projection_bound
    }

    pub fn enclosure_radius(&self) -> &Dyadic {
        &self.enclosure_radius
    }

    pub fn frequency(&self) -> CertifiedFrequency {
        self.frequency
    }
}

/// A shared eigenvalue's certified eigenpairs.
#[derive(Debug, PartialEq)]
pub struct CertifiedEigenspace {
    pub(crate) eigenvalue: f64,
    pub(crate) basis: Vec<CertifiedEigenpair>,
}

impl CertifiedEigenspace {
    pub fn eigenvalue(&self) -> f64 {
        self.eigenvalue
    }

    pub fn basis(&self) -> &[CertifiedEigenpair] {
        &self.basis
    }
}

/// The certified result of the top-level solve function.
#[derive(Debug, PartialEq)]
pub struct CertifiedEigenSolution {
    pub(crate) eigenspaces: Vec<CertifiedEigenspace>,
    pub(crate) convergence: ConvergenceStatus,
    pub(crate) projection_reference: CertifiedGeneralizedEigenInput,
    pub(crate) mass_lower_bound: Dyadic,
}

impl CertifiedEigenSolution {
    pub fn eigenspaces(&self) -> &[CertifiedEigenspace] {
        &self.eigenspaces
    }

    pub fn convergence(&self) -> ConvergenceStatus {
        self.convergence
    }

    pub fn projection_reference(&self) -> &CertifiedGeneralizedEigenInput {
        &self.projection_reference
    }

    /// The exact lower bound on the smallest eigenvalue of the mass matrix the
    /// solver received, computed once during certification.
    pub fn mass_lower_bound(&self) -> &Dyadic {
        &self.mass_lower_bound
    }
}

/// Typed failures returned by the top-level solve function.
#[derive(Debug, PartialEq)]
pub enum CertifiedEigenError {
    InvalidProblemShape(neco_generalized_eigen::GeneralizedEigenError),
    InvalidStiffness(SolverFailure),
    InvalidMass(SolverFailure),
    SolverInternal(SolverFailure),
    RequestedModesExceedDimension {
        requested: usize,
        dimension: usize,
    },
    ResidualNotConverged {
        convergence: ConvergenceStatus,
        non_convergent: Vec<NonConvergentEigenpair>,
    },
    Undeterminable {
        failures: Vec<UndeterminableEigenpair>,
    },
    ExactArithmetic(BigintError),
    Allocation(LinearError),
}

impl fmt::Display for CertifiedEigenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProblemShape(error) => error.fmt(formatter),
            Self::InvalidStiffness(error) => write!(formatter, "invalid stiffness: {error}"),
            Self::InvalidMass(error) => write!(formatter, "invalid mass: {error}"),
            Self::SolverInternal(error) => write!(formatter, "solver internal failure: {error}"),
            Self::RequestedModesExceedDimension {
                requested,
                dimension,
            } => write!(
                formatter,
                "requested {requested} modes exceeds problem dimension {dimension}"
            ),
            Self::ResidualNotConverged { non_convergent, .. } => write!(
                formatter,
                "{} eigenpair(s) did not converge",
                non_convergent.len()
            ),
            Self::Undeterminable { failures } => {
                write!(
                    formatter,
                    "{} eigenpair(s) could not be certified",
                    failures.len()
                )
            }
            Self::ExactArithmetic(error) => error.fmt(formatter),
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CertifiedEigenError {}
