use crate::{try_filled, try_workspace, EigensolveFaerError, RefinementFailureKind};
use faer::diag::DiagMut;
use faer::dyn_stack::MemStack;
use faer::linalg::matmul::matmul;
use faer::linalg::svd::{svd, svd_scratch, ComputeSvdVectors};
use faer::{c64, Accum, MatMut, MatRef, Par, Spec};
use neco_complex::Complex;

pub(super) const TRIGGER: f64 = 16.0 * f64::EPSILON;
pub(super) const ACCEPT: f64 = 1e-12;
const MAX_CONDITION: f64 = 1e8;

pub(super) struct Split {
    pub(super) rows: usize,
    pub(super) columns: usize,
    pub(super) re: Vec<f64>,
    pub(super) im: Vec<f64>,
}

impl Split {
    pub(super) fn zeros(rows: usize, columns: usize) -> Result<Self, EigensolveFaerError> {
        let elements = rows.saturating_mul(columns);
        Ok(Self {
            rows,
            columns,
            re: try_filled(elements, 0.0)?,
            im: try_filled(elements, 0.0)?,
        })
    }
}

pub(super) fn real_product(
    matrix: &[f64],
    dimension: usize,
    input: &[f64],
    columns: usize,
    output: &mut [f64],
) {
    matmul(
        MatMut::from_column_major_slice_mut(output, dimension, columns),
        Accum::Replace,
        MatRef::from_column_major_slice(matrix, dimension, dimension),
        MatRef::from_column_major_slice(input, dimension, columns),
        1.0,
        Par::Seq,
    );
}

pub(super) fn frobenius(matrix: &[f64]) -> f64 {
    matrix.iter().map(|value| value * value).sum::<f64>().sqrt()
}

pub(super) fn residual_ratios(
    matrix: &[f64],
    frobenius_norm: f64,
    eigenvalues: &[Complex<f64>],
    vectors: &Split,
) -> Result<(Vec<f64>, Split), EigensolveFaerError> {
    let rows = vectors.rows;
    let mut product = Split::zeros(rows, vectors.columns)?;
    real_product(matrix, rows, &vectors.re, vectors.columns, &mut product.re);
    real_product(matrix, rows, &vectors.im, vectors.columns, &mut product.im);
    let mut ratios = try_filled(vectors.columns, 0.0)?;
    for (column, ratio) in ratios.iter_mut().enumerate() {
        let (lambda_re, lambda_im) = (
            *eigenvalues[column].real(),
            *eigenvalues[column].imaginary(),
        );
        let mut residual = 0.0;
        let mut norm = 0.0;
        for row in 0..rows {
            let at = column * rows + row;
            let (xr, xi) = (vectors.re[at], vectors.im[at]);
            let er = product.re[at] - (lambda_re * xr - lambda_im * xi);
            let ei = product.im[at] - (lambda_re * xi + lambda_im * xr);
            residual += er * er + ei * ei;
            norm += xr * xr + xi * xi;
        }
        *ratio = residual.sqrt() / (frobenius_norm * norm.sqrt());
    }
    Ok((ratios, product))
}

pub(super) fn exceeds(value: f64, limit: f64) -> bool {
    value.is_nan() || value > limit
}

pub(super) fn failure(
    operation: &'static str,
    index: usize,
    residual: f64,
    kind: RefinementFailureKind,
) -> EigensolveFaerError {
    EigensolveFaerError::EigenvectorRefinement {
        operation,
        index,
        residual,
        kind,
    }
}

pub(super) fn replaces(before: f64, after: f64) -> bool {
    before.is_nan() || after < before
}

pub(super) fn condition_number(basis: &Split) -> Result<Option<f64>, EigensolveFaerError> {
    let (rows, columns) = (basis.rows, basis.columns);
    if columns < 2 {
        return Ok(Some(1.0));
    }
    let mut values = try_filled(rows * columns, c64::new(0.0, 0.0))?;
    for (value, (&re, &im)) in values.iter_mut().zip(basis.re.iter().zip(&basis.im)) {
        *value = c64::new(re, im);
    }
    let mut singular = try_filled(columns, c64::new(0.0, 0.0))?;
    let params = Spec::default();
    let mut workspace = try_workspace(svd_scratch::<c64>(
        rows,
        columns,
        ComputeSvdVectors::No,
        ComputeSvdVectors::No,
        Par::Seq,
        params,
    ))?;
    if svd(
        MatRef::from_column_major_slice(&values, rows, columns),
        DiagMut::from_slice_mut(&mut singular),
        None,
        None,
        Par::Seq,
        MemStack::new(&mut workspace),
        params,
    )
    .is_err()
    {
        return Ok(None);
    }
    Ok(Some(singular[0].re / singular[columns - 1].re))
}

pub(super) fn dependent(condition: f64) -> bool {
    exceeds(condition, MAX_CONDITION)
}

pub(super) struct Measured {
    pub(super) columns: Vec<usize>,
    pub(super) ratios: Vec<f64>,
}

pub(super) fn measure_selected(
    matrix: &[f64],
    frobenius_norm: f64,
    eigenvalues: &[Complex<f64>],
    partner: &[usize],
    vectors: &[Complex<f64>],
) -> Result<Measured, EigensolveFaerError> {
    let dimension = eigenvalues.len();
    let mut count = 0;
    for (column, lambda) in eigenvalues.iter().enumerate() {
        if partner[column] == column || *lambda.imaginary() > 0.0 {
            count += 1;
        }
    }
    let mut columns: Vec<usize> = try_filled(count, 0)?;
    let mut slot = 0;
    for (column, lambda) in eigenvalues.iter().enumerate() {
        if partner[column] == column || *lambda.imaginary() > 0.0 {
            columns[slot] = column;
            slot += 1;
        }
    }
    let mut chosen = Split::zeros(dimension, count)?;
    let mut chosen_lambda = try_filled(count, Complex::<f64>::zero())?;
    for (slot, &column) in columns.iter().enumerate() {
        chosen_lambda[slot] = eigenvalues[column];
        for row in 0..dimension {
            let value = vectors[column * dimension + row];
            chosen.re[slot * dimension + row] = *value.real();
            chosen.im[slot * dimension + row] = *value.imaginary();
        }
    }
    let (ratios, _) = residual_ratios(matrix, frobenius_norm, &chosen_lambda, &chosen)?;
    Ok(Measured { columns, ratios })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independence_follows_the_condition_number_of_the_columns() {
        let mut basis = Split::zeros(3, 2).unwrap();
        basis.re.copy_from_slice(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        assert!(!dependent(condition_number(&basis).unwrap().unwrap()));
        basis.re.copy_from_slice(&[1.0, 0.0, 0.0, 1.0, 1e-10, 0.0]);
        assert!(dependent(condition_number(&basis).unwrap().unwrap()));
    }

    #[test]
    fn a_rebuilt_vector_replaces_the_original_only_when_its_residual_is_smaller() {
        assert!(replaces(1e-9, 1e-15));
        assert!(!replaces(1e-15, 1e-9));
        assert!(!replaces(1e-12, 1e-12));
        assert!(replaces(f64::NAN, 1e-15));
    }
}
