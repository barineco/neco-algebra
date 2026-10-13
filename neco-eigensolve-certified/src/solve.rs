use neco_bigint::{BigintError, Dyadic};
use neco_eigensolve::{EigensolveConfig, EigensolveError, EigensolveRequest};
use neco_eigensolve_faer::EigensolveFaerError;
use neco_generalized_eigen::{ConvergenceStatus, Eigenpair, Eigenspace, GeneralizedEigenProblem};
use neco_linear_dense::DenseMatrix;
use neco_linear_exact::{CertifiedGeneralizedEigenInput, MassNormalizationMethod};
use neco_linear_types::{LinearError, LinearOperator, Vector};

use crate::bounds::{
    computational_bound, mass_lower_bound, projection_bound, MassLowerBoundError,
    ProjectionBoundError,
};
use crate::frequency::{certified_frequency, FrequencyCertificationError};
use crate::types::{
    CertifiedEigenError, CertifiedEigenSolution, CertifiedEigenpair, CertifiedEigenspace,
    NonConvergentEigenpair, SolverFailure, UndeterminableEigenpair, UndeterminableReason,
};

/// Computes certified forward-error bounds and a frequency enclosure for
/// each eigenpair, delegating to the identity or positive-definite solver
/// according to the mass normalization method.
pub fn solve_certified_generalized_eigen(
    input: CertifiedGeneralizedEigenInput,
    config: EigensolveConfig,
) -> Result<CertifiedEigenSolution, CertifiedEigenError> {
    let problem = GeneralizedEigenProblem::from_dense(
        input.stiffness().matrix().clone(),
        input.mass().matrix().matrix().clone(),
    )
    .map_err(CertifiedEigenError::InvalidProblemShape)?;

    if config.requested_modes() > problem.dimension() {
        return Err(CertifiedEigenError::RequestedModesExceedDimension {
            requested: config.requested_modes(),
            dimension: problem.dimension(),
        });
    }

    let method = input.mass().method();
    let (eigenspaces, convergence, input): (
        Vec<Eigenspace>,
        ConvergenceStatus,
        CertifiedGeneralizedEigenInput,
    ) = match method {
        MassNormalizationMethod::Identity => {
            let request = EigensolveRequest::new(problem, config, input);
            let result = neco_eigensolve::solve_request_symmetric_f64(request)
                .map_err(classify_identity_failure)?;
            let eigenspaces = result.eigenspaces().to_vec();
            let convergence = result.convergence();
            (eigenspaces, convergence, result.into_projection_reference())
        }
        MassNormalizationMethod::PositiveDefiniteCholesky => {
            let request = EigensolveRequest::new(problem, config, input);
            let result = neco_eigensolve_faer::solve_request_symmetric_f64(request)
                .map_err(classify_positive_definite_failure)?;
            let eigenspaces = result.eigenspaces().to_vec();
            let convergence = result.convergence();
            (eigenspaces, convergence, result.into_projection_reference())
        }
    };

    let non_convergent: Vec<NonConvergentEigenpair> = eigenspaces
        .iter()
        .flat_map(Eigenspace::basis)
        .filter(|pair| {
            !pair
                .residual()
                .meets_tolerance(config.absolute_tolerance(), config.relative_tolerance())
        })
        .map(|pair| NonConvergentEigenpair {
            eigenvalue: pair.eigenvalue(),
            residual: pair.residual(),
        })
        .collect();
    if !non_convergent.is_empty() {
        return Err(CertifiedEigenError::ResidualNotConverged {
            convergence,
            non_convergent,
        });
    }

    let stiffness = input.stiffness().matrix();
    let mass = input.mass().matrix().matrix();

    let sigma = match mass_lower_bound(mass, method) {
        Ok(sigma) => sigma,
        Err(MassLowerBoundError::NotPositive) => {
            return Err(all_undeterminable(
                &eigenspaces,
                UndeterminableReason::MassLowerBoundUnavailable,
            ));
        }
        Err(MassLowerBoundError::Bigint(error)) => {
            return Err(CertifiedEigenError::ExactArithmetic(error));
        }
    };

    let mut certified_eigenspaces = Vec::with_capacity(eigenspaces.len());
    let mut failures = Vec::new();
    for eigenspace in eigenspaces {
        let eigenvalue = eigenspace.eigenvalue();
        let mut basis = Vec::with_capacity(eigenspace.basis().len());
        for pair in eigenspace.basis() {
            match certify_eigenpair(stiffness, mass, method, &sigma, &input, pair.clone()) {
                Ok(certified_pair) => basis.push(certified_pair),
                Err(EigenpairCertificationError::Undeterminable {
                    reason,
                    enclosure_radius,
                }) => failures.push(UndeterminableEigenpair {
                    eigenvalue,
                    enclosure_radius,
                    reason,
                }),
                Err(EigenpairCertificationError::ExactArithmetic(error)) => {
                    return Err(CertifiedEigenError::ExactArithmetic(error));
                }
                Err(EigenpairCertificationError::Allocation(error)) => {
                    return Err(CertifiedEigenError::Allocation(error));
                }
                Err(EigenpairCertificationError::SolverInternal(error)) => {
                    return Err(CertifiedEigenError::SolverInternal(error));
                }
            }
        }
        if !basis.is_empty() {
            certified_eigenspaces.push(CertifiedEigenspace { eigenvalue, basis });
        }
    }

    if !failures.is_empty() {
        return Err(CertifiedEigenError::Undeterminable { failures });
    }

    Ok(CertifiedEigenSolution {
        eigenspaces: certified_eigenspaces,
        convergence,
        projection_reference: input,
        mass_lower_bound: sigma,
    })
}

fn classify_identity_failure(error: EigensolveError) -> CertifiedEigenError {
    match &error {
        EigensolveError::InvalidStiffnessMatrix { .. } => {
            CertifiedEigenError::InvalidStiffness(SolverFailure::IdentityMass(error))
        }
        EigensolveError::UnsupportedMassMatrix { .. } => {
            CertifiedEigenError::InvalidMass(SolverFailure::IdentityMass(error))
        }
        _ => CertifiedEigenError::SolverInternal(SolverFailure::IdentityMass(error)),
    }
}

fn classify_positive_definite_failure(error: EigensolveFaerError) -> CertifiedEigenError {
    match &error {
        EigensolveFaerError::InvalidStiffnessMatrix { .. } => {
            CertifiedEigenError::InvalidStiffness(SolverFailure::PositiveDefiniteMass(error))
        }
        EigensolveFaerError::InvalidMassMatrix { .. }
        | EigensolveFaerError::NonPositiveMassMatrix => {
            CertifiedEigenError::InvalidMass(SolverFailure::PositiveDefiniteMass(error))
        }
        _ => CertifiedEigenError::SolverInternal(SolverFailure::PositiveDefiniteMass(error)),
    }
}

fn all_undeterminable(
    eigenspaces: &[Eigenspace],
    reason: UndeterminableReason,
) -> CertifiedEigenError {
    let failures = eigenspaces
        .iter()
        .flat_map(Eigenspace::basis)
        .map(|pair| UndeterminableEigenpair {
            eigenvalue: pair.eigenvalue(),
            enclosure_radius: None,
            reason,
        })
        .collect();
    CertifiedEigenError::Undeterminable { failures }
}

enum EigenpairCertificationError {
    Undeterminable {
        reason: UndeterminableReason,
        enclosure_radius: Option<f64>,
    },
    ExactArithmetic(BigintError),
    Allocation(LinearError),
    SolverInternal(SolverFailure),
}

impl From<BigintError> for EigenpairCertificationError {
    fn from(error: BigintError) -> Self {
        Self::ExactArithmetic(error)
    }
}

impl From<LinearError> for EigenpairCertificationError {
    fn from(error: LinearError) -> Self {
        Self::Allocation(error)
    }
}

fn certify_eigenpair(
    stiffness: &DenseMatrix<f64>,
    mass: &DenseMatrix<f64>,
    method: MassNormalizationMethod,
    sigma: &Dyadic,
    input: &CertifiedGeneralizedEigenInput,
    pair: Eigenpair,
) -> Result<CertifiedEigenpair, EigenpairCertificationError> {
    let eta_k = input.stiffness().absolute_error_bound();
    let eta_m = input.mass().absolute_error_bound();
    let z = approximate_correction(stiffness, mass, method, &pair)?;

    let bound = computational_bound(
        stiffness,
        mass,
        pair.eigenvalue(),
        pair.eigenvector(),
        sigma,
        &z,
    )?;

    let projection = match projection_bound(eta_k, eta_m, sigma, pair.eigenvalue(), &bound) {
        Ok(bound) => bound,
        Err(ProjectionBoundError::Bigint(error)) => return Err(error.into()),
        Err(ProjectionBoundError::PerturbationTooLarge) => {
            return Err(EigenpairCertificationError::Undeterminable {
                reason: UndeterminableReason::ProjectionPerturbationTooLarge,
                enclosure_radius: None,
            });
        }
    };

    let enclosure_radius = bound.add(&projection)?;

    let frequency = match certified_frequency(pair.eigenvalue(), &enclosure_radius) {
        Ok(frequency) => frequency,
        Err(FrequencyCertificationError::Undeterminable(reason)) => {
            let enclosure_radius_estimate = enclosure_radius.round_to_f64_ties_even().ok();
            return Err(EigenpairCertificationError::Undeterminable {
                reason,
                enclosure_radius: enclosure_radius_estimate,
            });
        }
        Err(FrequencyCertificationError::ExactArithmetic(error)) => return Err(error.into()),
    };

    Ok(CertifiedEigenpair {
        pair,
        computational_bound: bound,
        projection_bound: projection,
        enclosure_radius,
        frequency,
    })
}

fn approximate_correction(
    stiffness: &DenseMatrix<f64>,
    mass: &DenseMatrix<f64>,
    method: MassNormalizationMethod,
    pair: &Eigenpair,
) -> Result<Vector<f64>, EigenpairCertificationError> {
    match method {
        MassNormalizationMethod::Identity => Ok(Vector::try_zeros(stiffness.rows(), 0.0)?),
        MassNormalizationMethod::PositiveDefiniteCholesky => {
            let stiffness_product = stiffness.apply(pair.eigenvector())?;
            let mass_product = mass.apply(pair.eigenvector())?;
            let eigenvalue = pair.eigenvalue();
            let residual_values: Vec<f64> = stiffness_product
                .values()
                .iter()
                .zip(mass_product.values())
                .map(|(stiffness_value, mass_value)| stiffness_value - eigenvalue * mass_value)
                .collect();
            let residual = Vector::try_from_vec(residual_values)?;
            neco_eigensolve_faer::approximate_mass_solve(mass, &residual).map_err(|error| {
                EigenpairCertificationError::SolverInternal(SolverFailure::PositiveDefiniteMass(
                    error,
                ))
            })
        }
    }
}
