use neco_bigint::{BigintError, Dyadic};
use neco_linear_dense::DenseMatrix;
use neco_linear_exact::MassNormalizationMethod;
use neco_linear_types::Vector;

const MAX_BOUND_SEARCH_STEPS: u32 = 4096;

#[derive(Debug)]
pub(crate) enum MassLowerBoundError {
    NotPositive,
    Bigint(BigintError),
}

impl From<BigintError> for MassLowerBoundError {
    fn from(error: BigintError) -> Self {
        Self::Bigint(error)
    }
}

pub(crate) fn mass_lower_bound(
    mass: &DenseMatrix<f64>,
    method: MassNormalizationMethod,
) -> Result<Dyadic, MassLowerBoundError> {
    match method {
        MassNormalizationMethod::Identity => Ok(Dyadic::from_f64_exact(1.0)?),
        MassNormalizationMethod::PositiveDefiniteCholesky => {
            let dimension = mass.rows();
            let values = mass.values();
            let mut minimum: Option<Dyadic> = None;
            for row in 0..dimension {
                let mut off_diagonal_sum = Dyadic::from_f64_exact(0.0)?;
                let mut diagonal = Dyadic::from_f64_exact(0.0)?;
                for column in 0..dimension {
                    let value = values[column * dimension + row];
                    if column == row {
                        diagonal = Dyadic::from_f64_exact(value)?;
                    } else {
                        off_diagonal_sum =
                            off_diagonal_sum.add(&Dyadic::from_f64_exact(value.abs())?)?;
                    }
                }
                let row_bound = diagonal.sub(&off_diagonal_sum)?;
                minimum = Some(match minimum {
                    None => row_bound,
                    Some(current) if current <= row_bound => current,
                    Some(_) => row_bound,
                });
            }
            let sigma = minimum.ok_or(MassLowerBoundError::NotPositive)?;
            let zero = Dyadic::from_f64_exact(0.0)?;
            if sigma <= zero {
                return Err(MassLowerBoundError::NotPositive);
            }
            Ok(sigma)
        }
    }
}

pub(crate) fn computational_bound(
    stiffness: &DenseMatrix<f64>,
    mass: &DenseMatrix<f64>,
    eigenvalue: f64,
    eigenvector: &Vector<f64>,
    sigma: &Dyadic,
    z: &Vector<f64>,
) -> Result<Dyadic, BigintError> {
    let dimension = eigenvector.len();
    let x = dyadic_from_f64_slice(eigenvector.values())?;
    let z_dyadic = dyadic_from_f64_slice(z.values())?;
    let mu = Dyadic::from_f64_exact(eigenvalue)?;

    let kx = dyadic_matrix_vector_product(stiffness, dimension, &x)?;
    let mx = dyadic_matrix_vector_product(mass, dimension, &x)?;
    let mut r = Vec::with_capacity(dimension);
    for (k_value, m_value) in kx.iter().zip(&mx) {
        r.push(k_value.sub(&mu.mul(m_value)?)?);
    }

    let nu = dyadic_dot(&x, &mx)?;

    let mz = dyadic_matrix_vector_product(mass, dimension, &z_dyadic)?;
    let mut s = Vec::with_capacity(dimension);
    for (r_value, mz_value) in r.iter().zip(&mz) {
        s.push(r_value.sub(mz_value)?);
    }

    let z_m_z = dyadic_dot(&z_dyadic, &mz)?;
    let z_s = dyadic_dot(&z_dyadic, &s)?;
    let s_s = dyadic_dot(&s, &s)?;

    let two = Dyadic::from_f64_exact(2.0)?;
    let inner = z_m_z.add(&two.mul(&z_s)?)?;
    let rhs = sigma.mul(&inner)?.add(&s_s)?;
    let sigma_nu = sigma.mul(&nu)?;

    certify_quadratic_bound(&rhs, &sigma_nu)
}

pub(crate) fn dyadic_from_f64_slice(values: &[f64]) -> Result<Vec<Dyadic>, BigintError> {
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        result.push(Dyadic::from_f64_exact(*value)?);
    }
    Ok(result)
}

pub(crate) fn dyadic_matrix_vector_product(
    matrix: &DenseMatrix<f64>,
    dimension: usize,
    vector: &[Dyadic],
) -> Result<Vec<Dyadic>, BigintError> {
    let values = matrix.values();
    let mut result = Vec::with_capacity(dimension);
    for row in 0..dimension {
        let mut sum = Dyadic::from_f64_exact(0.0)?;
        for column in 0..dimension {
            let entry = Dyadic::from_f64_exact(values[column * dimension + row])?;
            sum = sum.add(&entry.mul(&vector[column])?)?;
        }
        result.push(sum);
    }
    Ok(result)
}

pub(crate) fn dyadic_dot(left: &[Dyadic], right: &[Dyadic]) -> Result<Dyadic, BigintError> {
    let mut sum = Dyadic::from_f64_exact(0.0)?;
    for (left_value, right_value) in left.iter().zip(right) {
        sum = sum.add(&left_value.mul(right_value)?)?;
    }
    Ok(sum)
}

pub(crate) fn certify_quadratic_bound(
    rhs: &Dyadic,
    product: &Dyadic,
) -> Result<Dyadic, BigintError> {
    let zero = Dyadic::from_f64_exact(0.0)?;
    if *rhs <= zero {
        return Ok(zero);
    }
    let rhs_estimate = rhs.round_to_f64_ties_even().unwrap_or(f64::INFINITY);
    let product_estimate = product.round_to_f64_ties_even().unwrap_or(0.0);
    let mut candidate = if product_estimate > 0.0 {
        (rhs_estimate / product_estimate).sqrt()
    } else {
        0.0
    };
    if !candidate.is_finite() || candidate < 0.0 {
        candidate = 0.0;
    }
    for _ in 0..MAX_BOUND_SEARCH_STEPS {
        let candidate_dyadic = Dyadic::from_f64_exact(candidate)?;
        let lhs = candidate_dyadic.mul(&candidate_dyadic)?.mul(product)?;
        if lhs >= *rhs {
            return Ok(candidate_dyadic);
        }
        candidate = next_up_nonnegative(candidate);
    }
    Err(BigintError::FloatOutOfRange)
}

pub(crate) fn certify_linear_bound(
    numerator: &Dyadic,
    denominator: &Dyadic,
) -> Result<Dyadic, BigintError> {
    let zero = Dyadic::from_f64_exact(0.0)?;
    if *numerator <= zero {
        return Ok(zero);
    }
    let numerator_estimate = numerator.round_to_f64_ties_even().unwrap_or(f64::INFINITY);
    let denominator_estimate = denominator.round_to_f64_ties_even().unwrap_or(0.0);
    let mut candidate = if denominator_estimate > 0.0 {
        numerator_estimate / denominator_estimate
    } else {
        0.0
    };
    if !candidate.is_finite() || candidate < 0.0 {
        candidate = 0.0;
    }
    for _ in 0..MAX_BOUND_SEARCH_STEPS {
        let candidate_dyadic = Dyadic::from_f64_exact(candidate)?;
        let lhs = candidate_dyadic.mul(denominator)?;
        if lhs >= *numerator {
            return Ok(candidate_dyadic);
        }
        candidate = next_up_nonnegative(candidate);
    }
    Err(BigintError::FloatOutOfRange)
}

pub(crate) fn next_up_nonnegative(value: f64) -> f64 {
    if value <= 0.0 {
        f64::from_bits(1)
    } else {
        f64::from_bits(value.to_bits() + 1)
    }
}

#[derive(Debug)]
pub(crate) enum ProjectionBoundError {
    PerturbationTooLarge,
    Bigint(BigintError),
}

impl From<BigintError> for ProjectionBoundError {
    fn from(error: BigintError) -> Self {
        Self::Bigint(error)
    }
}

pub(crate) fn projection_bound(
    eta_k: &Dyadic,
    eta_m: &Dyadic,
    sigma: &Dyadic,
    mu: f64,
    delta_comp: &Dyadic,
) -> Result<Dyadic, ProjectionBoundError> {
    let two = Dyadic::from_f64_exact(2.0)?;
    let two_eta_m = two.mul(eta_m)?;
    if two_eta_m > *sigma {
        return Err(ProjectionBoundError::PerturbationTooLarge);
    }
    let denominator = sigma.sub(eta_m)?;
    let mu_term = Dyadic::from_f64_exact(mu.abs())?.add(delta_comp)?;
    let numerator = eta_k.add(&eta_m.mul(&mu_term)?)?;
    Ok(certify_linear_bound(&numerator, &denominator)?)
}

#[cfg(test)]
mod tests {
    use super::{mass_lower_bound, projection_bound, MassLowerBoundError, ProjectionBoundError};
    use crate::bounds::computational_bound;
    use neco_bigint::Dyadic;
    use neco_linear_dense::DenseMatrix;
    use neco_linear_exact::MassNormalizationMethod;
    use neco_linear_types::{Shape, Vector};

    fn dense(values: Vec<f64>) -> DenseMatrix<f64> {
        DenseMatrix::from_row_major(Shape::new(2, 2), values).expect("matrix")
    }

    fn vector(values: Vec<f64>) -> Vector<f64> {
        Vector::try_from_vec(values).expect("vector")
    }

    fn dyadic(value: f64) -> Dyadic {
        Dyadic::from_f64_exact(value).expect("finite value")
    }

    fn assert_close(actual: f64, expected: f64) {
        let relative = ((actual - expected) / expected).abs();
        assert!(
            relative <= 1.0e-9,
            "expected {expected}, got {actual} (relative error {relative})"
        );
    }

    #[test]
    fn identity_mass_lower_bound_is_exactly_one() {
        let sigma = mass_lower_bound(
            &dense(vec![1.0, 0.0, 0.0, 1.0]),
            MassNormalizationMethod::Identity,
        )
        .expect("sigma");
        assert_eq!(sigma, dyadic(1.0));
    }

    #[test]
    fn mass_lower_bound_fails_for_non_diagonally_dominant_positive_definite_mass() {
        let mass = dense(vec![1.0, 1.5, 1.5, 4.0]);
        assert!(matches!(
            mass_lower_bound(&mass, MassNormalizationMethod::PositiveDefiniteCholesky),
            Err(MassLowerBoundError::NotPositive)
        ));
    }

    #[test]
    fn mass_lower_bound_uses_absolute_value_of_negative_off_diagonal_entries() {
        let mass = dense(vec![2.0, -1.0, -1.0, 2.0]);
        let sigma = mass_lower_bound(&mass, MassNormalizationMethod::PositiveDefiniteCholesky)
            .expect("sigma");
        assert_eq!(sigma, dyadic(1.0));
    }

    #[test]
    fn computational_bound_certifies_despite_zero_f64_residual() {
        let stiffness = dense(vec![
            90448891.05823874,
            34268656.26802122,
            34268656.26802122,
            83253967.10105884,
        ]);
        let mass = dense(vec![1.0, 0.0, 0.0, 1.0]);
        let eigenvector = vector(vec![0.6691769238424816, -0.7431031184140687]);
        let eigenvalue = 52394462.656250596;
        let sigma = dyadic(1.0);
        let zero = vector(vec![0.0, 0.0]);

        let residual = neco_generalized_eigen::EigenResidual::from_problem(
            &neco_generalized_eigen::GeneralizedEigenProblem::from_dense(
                stiffness.clone(),
                mass.clone(),
            )
            .expect("problem"),
            eigenvalue,
            &eigenvector,
        )
        .expect("residual");
        assert_eq!(residual.absolute(), 0.0);

        let bound = computational_bound(&stiffness, &mass, eigenvalue, &eigenvector, &sigma, &zero)
            .expect("bound");
        let bound_f64 = bound.round_to_f64_ties_even().expect("finite bound");
        assert!(bound_f64 > 0.0);
        assert_close(bound_f64, 6.675047701647799e-09);
    }

    #[test]
    fn computational_bound_uses_the_z_correction_for_non_identity_mass() {
        let stiffness = dense(vec![5.0, 2.0, 2.0, 5.0]);
        let mass = dense(vec![1.0, 0.0, 0.0, 4.0]);
        let eigenvector = vector(vec![0.99, 0.1]);
        let eigenvalue = 4.9;
        let sigma = dyadic(1.0);
        let z = vector(vec![0.2990000000000004, 0.12999999999999995]);

        let bound = computational_bound(&stiffness, &mass, eigenvalue, &eigenvector, &sigma, &z)
            .expect("bound");
        let bound_f64 = bound.round_to_f64_ties_even().expect("finite bound");
        assert_close(bound_f64, 0.3923104128785467);
    }

    #[test]
    fn projection_bound_grows_with_projection_error_and_rejects_large_mass_perturbation() {
        let eta_k = dyadic(0.1);
        let eta_m = dyadic(0.05);
        let sigma = dyadic(1.0);
        let delta_comp = dyadic(0.001);
        let bound = projection_bound(&eta_k, &eta_m, &sigma, 4.0, &delta_comp).expect("bound");
        let bound_f64 = bound.round_to_f64_ties_even().expect("finite bound");
        assert_close(bound_f64, 0.3158421052631579);

        let large_eta_m = dyadic(0.6);
        assert!(matches!(
            projection_bound(&eta_k, &large_eta_m, &sigma, 4.0, &delta_comp),
            Err(ProjectionBoundError::PerturbationTooLarge)
        ));
    }

    #[test]
    fn projection_bound_separates_stiffness_and_mass_projection_error() {
        let stiffness_only =
            projection_bound(&dyadic(0.2), &dyadic(0.0), &dyadic(1.0), 4.0, &dyadic(0.0))
                .expect("bound");
        assert_close(
            stiffness_only.round_to_f64_ties_even().expect("finite"),
            0.2,
        );
        let mass_only =
            projection_bound(&dyadic(0.0), &dyadic(0.1), &dyadic(1.0), 4.0, &dyadic(0.0))
                .expect("bound");
        assert_close(
            mass_only.round_to_f64_ties_even().expect("finite"),
            0.1 * 4.0 / 0.9,
        );
        assert_ne!(stiffness_only, mass_only);
    }
}
