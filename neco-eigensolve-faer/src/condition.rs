use crate::convert::{complex_scale_exponent, scaled_faer_values, validate_square_complex};
use crate::{try_filled, try_workspace, EigensolveFaerError};
use faer::diag::DiagMut;
use faer::dyn_stack::MemStack;
use faer::linalg::svd::{svd, svd_scratch, ComputeSvdVectors};
use faer::{c64, MatRef, Par, Spec};
use neco_complex::Complex;
use neco_linear_dense::DenseMatrix;

const OPERATION: &str = "condition_number_2";

pub fn condition_number_2(matrix: &DenseMatrix<Complex<f64>>) -> Result<f64, EigensolveFaerError> {
    let dimension = validate_square_complex(matrix, OPERATION)?;
    if matrix
        .values()
        .iter()
        .all(|value| *value.real() == 0.0 && *value.imaginary() == 0.0)
    {
        return Err(EigensolveFaerError::InvalidMatrix {
            operation: OPERATION,
            reason: "matrix is zero",
        });
    }
    let values = scaled_faer_values(matrix.values(), -complex_scale_exponent(matrix.values()))?;
    let mut singular_values = try_filled(dimension, c64::new(0.0, 0.0))?;
    let params = Spec::default();
    let mut workspace = try_workspace(svd_scratch::<c64>(
        dimension,
        dimension,
        ComputeSvdVectors::No,
        ComputeSvdVectors::No,
        Par::Seq,
        params,
    ))?;
    svd(
        MatRef::from_column_major_slice(&values, dimension, dimension),
        DiagMut::from_slice_mut(&mut singular_values),
        None,
        None,
        Par::Seq,
        MemStack::new(&mut workspace),
        params,
    )
    .map_err(|_| EigensolveFaerError::ExternalSolver {
        operation: OPERATION,
        reason: "singular value decomposition did not converge",
    })?;
    let largest = singular_values[0].re;
    let smallest = singular_values[dimension - 1].re;
    if smallest == 0.0 {
        return Ok(f64::INFINITY);
    }
    Ok(largest / smallest)
}
