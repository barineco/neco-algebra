use crate::{try_filled, EigensolveFaerError};
use faer::c64;
use neco_complex::Complex;
use neco_linear_dense::DenseMatrix;

fn invalid(operation: &'static str, reason: &'static str) -> EigensolveFaerError {
    EigensolveFaerError::InvalidMatrix { operation, reason }
}

pub(crate) fn validate_finite_complex(
    values: &[Complex<f64>],
    operation: &'static str,
) -> Result<(), EigensolveFaerError> {
    if values
        .iter()
        .all(|value| value.real().is_finite() && value.imaginary().is_finite())
    {
        Ok(())
    } else {
        Err(invalid(operation, "matrix has a non-finite element"))
    }
}

pub(crate) fn validate_square_real(
    matrix: &DenseMatrix<f64>,
    operation: &'static str,
) -> Result<usize, EigensolveFaerError> {
    if matrix.rows() != matrix.columns() {
        return Err(invalid(operation, "matrix is not square"));
    }
    if matrix.rows() == 0 {
        return Err(invalid(operation, "matrix is empty"));
    }
    if !matrix.values().iter().all(|value| value.is_finite()) {
        return Err(invalid(operation, "matrix has a non-finite element"));
    }
    Ok(matrix.rows())
}

pub(crate) fn validate_square_complex(
    matrix: &DenseMatrix<Complex<f64>>,
    operation: &'static str,
) -> Result<usize, EigensolveFaerError> {
    if matrix.rows() != matrix.columns() {
        return Err(invalid(operation, "matrix is not square"));
    }
    if matrix.rows() == 0 {
        return Err(invalid(operation, "matrix is empty"));
    }
    validate_finite_complex(matrix.values(), operation)?;
    Ok(matrix.rows())
}

/// Exponent of the power of two nearest below the largest finite component
/// magnitude; 0 for an all-zero input.
pub(crate) fn scale_exponent(components: impl Iterator<Item = f64>) -> i32 {
    let largest = components.fold(0.0, |current: f64, value| current.max(value.abs()));
    if largest == 0.0 {
        return 0;
    }
    let mut value = largest;
    let mut offset = 0;
    while value < f64::MIN_POSITIVE {
        value *= 18014398509481984.0;
        offset -= 54;
    }
    ((value.to_bits() >> 52) & 0x7ff) as i32 - 1023 + offset
}

pub(crate) fn complex_scale_exponent(values: &[Complex<f64>]) -> i32 {
    scale_exponent(
        values
            .iter()
            .flat_map(|value| [*value.real(), *value.imaginary()]),
    )
}

pub(crate) fn real_scale_exponent(values: &[f64]) -> i32 {
    scale_exponent(values.iter().copied())
}

/// Multiplication by 2^exponent in exact steps; a power of two changes no
/// significand bits unless the result leaves the normal range.
pub(crate) fn scale_by_power_of_two(value: f64, exponent: i32) -> f64 {
    let mut value = value;
    let mut remaining = exponent;
    while remaining != 0 {
        let step = remaining.clamp(-512, 512);
        value *= f64::from_bits(((1023 + step) as u64) << 52);
        remaining -= step;
    }
    value
}

pub(crate) fn to_faer_values(values: &[Complex<f64>]) -> Result<Vec<c64>, EigensolveFaerError> {
    let mut converted = try_filled(values.len(), c64::new(0.0, 0.0))?;
    for (target, source) in converted.iter_mut().zip(values) {
        *target = c64::new(*source.real(), *source.imaginary());
    }
    Ok(converted)
}

pub(crate) fn scaled_faer_values(
    values: &[Complex<f64>],
    exponent: i32,
) -> Result<Vec<c64>, EigensolveFaerError> {
    let mut converted = try_filled(values.len(), c64::new(0.0, 0.0))?;
    for (target, source) in converted.iter_mut().zip(values) {
        *target = c64::new(
            scale_by_power_of_two(*source.real(), exponent),
            scale_by_power_of_two(*source.imaginary(), exponent),
        );
    }
    Ok(converted)
}

pub(crate) fn scaled_real_values(
    values: &[f64],
    exponent: i32,
) -> Result<Vec<f64>, EigensolveFaerError> {
    let mut converted = try_filled(values.len(), 0.0)?;
    for (target, source) in converted.iter_mut().zip(values) {
        *target = scale_by_power_of_two(*source, exponent);
    }
    Ok(converted)
}

pub(crate) fn from_faer_values(values: &[c64]) -> Result<Vec<Complex<f64>>, EigensolveFaerError> {
    let mut converted = try_filled(values.len(), Complex::<f64>::zero())?;
    for (target, source) in converted.iter_mut().zip(values) {
        *target = Complex::new(source.re, source.im);
    }
    Ok(converted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_round_trips_components() {
        let values = vec![Complex::new(1.5, -2.5), Complex::new(-0.0, 4.0)];
        let converted = to_faer_values(&values).unwrap();
        assert_eq!(converted[0].re, 1.5);
        assert_eq!(converted[0].im, -2.5);
        let restored = from_faer_values(&converted).unwrap();
        assert_eq!(restored, values);
        assert!(restored[1].real().is_sign_negative());
    }

    #[test]
    fn finite_check_rejects_each_component() {
        assert!(validate_finite_complex(&[Complex::new(1.0, 2.0)], "x").is_ok());
        assert!(validate_finite_complex(&[Complex::new(f64::NAN, 0.0)], "x").is_err());
        assert!(validate_finite_complex(&[Complex::new(0.0, f64::INFINITY)], "x").is_err());
    }

    #[test]
    fn exponent_places_the_largest_component_in_one_to_two() {
        for largest in [1e-300, 3e-320, 0.75, 1.0, 1.999, 2.0, 1e150, 1e300] {
            let exponent = real_scale_exponent(&[largest, -largest / 3.0]);
            let scaled = scale_by_power_of_two(largest, -exponent);
            assert!((1.0..2.0).contains(&scaled), "{largest:e}: {scaled}");
        }
        assert_eq!(real_scale_exponent(&[0.0, -0.0]), 0);
    }

    #[test]
    fn scaling_by_a_power_of_two_round_trips_exactly() {
        for value in [1.0, -0.3, 123456.789, 1e300, 1e-300] {
            for exponent in [-1000, -513, -1, 0, 1, 512, 1000] {
                let scaled = scale_by_power_of_two(value, exponent);
                if scaled.is_normal() {
                    assert_eq!(scale_by_power_of_two(scaled, -exponent), value);
                }
            }
        }
    }
}
