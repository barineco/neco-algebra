use super::residual::{real_product, Split, TRIGGER};
use crate::{try_filled, try_workspace, EigensolveFaerError};
use faer::diag::DiagMut;
use faer::dyn_stack::{MemStack, StackReq};
use faer::linalg::evd::hessenberg::{
    hessenberg_in_place, hessenberg_in_place_scratch, HessenbergParams,
};
use faer::linalg::evd::{evd_cplx, evd_scratch, ComputeEigenvectors};
use faer::linalg::householder::{
    apply_block_householder_sequence_on_the_right_in_place_scratch,
    apply_block_householder_sequence_on_the_right_in_place_with_conj,
};
use faer::linalg::matmul::matmul;
use faer::{c64, Accum, Conj, MatMut, MatRef, Par, Spec};
use neco_complex::Complex;

const MAX_ITERATIONS: usize = 8;
const DEPENDENCE_RATIO: f64 = 1e-8;

pub(super) struct Hessenberg {
    pub(super) dimension: usize,
    h: Vec<f64>,
    q: Vec<f64>,
}

impl Hessenberg {
    pub(super) fn new(matrix: &[f64], dimension: usize) -> Result<Self, EigensolveFaerError> {
        let n = dimension;
        let par = Par::Seq;
        let block =
            faer::linalg::qr::no_pivoting::factor::recommended_block_size::<f64>(n - 1, n - 1);
        let params = Spec::<HessenbergParams, f64>::default();
        let request = StackReq::any_of(&[
            hessenberg_in_place_scratch::<f64>(n, block, par, params),
            apply_block_householder_sequence_on_the_right_in_place_scratch::<f64>(
                n - 1,
                block,
                n - 1,
            ),
        ]);
        let mut workspace = try_workspace(request)?;
        let stack = MemStack::new(&mut workspace);
        let mut h = try_filled(n * n, 0.0)?;
        h.copy_from_slice(matrix);
        let mut householder = try_filled(block * (n - 1), 0.0)?;
        hessenberg_in_place(
            MatMut::from_column_major_slice_mut(&mut h, n, n),
            MatMut::from_column_major_slice_mut(&mut householder, block, n - 1),
            par,
            stack,
            params,
        );
        let mut q = try_filled(n * n, 0.0)?;
        for index in 0..n {
            q[index * n + index] = 1.0;
        }
        apply_block_householder_sequence_on_the_right_in_place_with_conj(
            MatRef::from_column_major_slice(&h, n, n).submatrix(1, 0, n - 1, n - 1),
            MatRef::from_column_major_slice(&householder, block, n - 1),
            Conj::No,
            MatMut::from_column_major_slice_mut(&mut q, n, n).submatrix_mut(1, 1, n - 1, n - 1),
            par,
            stack,
        );
        for column in 0..n {
            for row in column + 2..n {
                h[column * n + row] = 0.0;
            }
        }
        Ok(Self { dimension, h, q })
    }
}

fn divide(numerator: Complex<f64>, denominator: Complex<f64>) -> Complex<f64> {
    let (dr, di) = (*denominator.real(), *denominator.imaginary());
    let (nr, ni) = (*numerator.real(), *numerator.imaginary());
    if dr.abs() >= di.abs() {
        let ratio = di / dr;
        let scale = dr + di * ratio;
        Complex::new((nr + ni * ratio) / scale, (ni - nr * ratio) / scale)
    } else {
        let ratio = dr / di;
        let scale = dr * ratio + di;
        Complex::new((nr * ratio + ni) / scale, (ni * ratio - nr) / scale)
    }
}

struct HessenbergLu {
    dimension: usize,
    upper: Vec<Complex<f64>>,
    multipliers: Vec<Complex<f64>>,
    swapped: Vec<bool>,
}

impl HessenbergLu {
    fn new(
        hessenberg: &Hessenberg,
        shift: Complex<f64>,
        floor: f64,
    ) -> Result<Self, EigensolveFaerError> {
        let n = hessenberg.dimension;
        let mut upper = try_filled(n * n, Complex::<f64>::zero())?;
        for row in 0..n {
            for column in row.saturating_sub(1)..n {
                upper[row * n + column] = Complex::new(hessenberg.h[column * n + row], 0.0);
            }
            upper[row * n + row] = Complex::new(
                hessenberg.h[row * n + row] - shift.real(),
                -shift.imaginary(),
            );
        }
        let mut multipliers = try_filled(n, Complex::<f64>::zero())?;
        let mut swapped = try_filled(n, false)?;
        for step in 0..n - 1 {
            let (here, below) = (
                upper[step * n + step].norm(),
                upper[(step + 1) * n + step].norm(),
            );
            if below > here {
                swapped[step] = true;
                for column in step..n {
                    upper.swap(step * n + column, (step + 1) * n + column);
                }
            }
            let mut pivot = upper[step * n + step];
            if pivot.norm() < floor {
                pivot = Complex::new(floor, 0.0);
                upper[step * n + step] = pivot;
            }
            let multiplier = divide(upper[(step + 1) * n + step], pivot);
            multipliers[step] = multiplier;
            upper[(step + 1) * n + step] = Complex::<f64>::zero();
            for column in step + 1..n {
                let change = multiplier * upper[step * n + column];
                upper[(step + 1) * n + column] -= change;
            }
        }
        if upper[(n - 1) * n + n - 1].norm() < floor {
            upper[(n - 1) * n + n - 1] = Complex::new(floor, 0.0);
        }
        Ok(Self {
            dimension: n,
            upper,
            multipliers,
            swapped,
        })
    }

    fn solve(&self, columns: &mut [Vec<Complex<f64>>]) -> Result<(), EigensolveFaerError> {
        let n = self.dimension;
        for step in 0..n - 1 {
            for column in columns.iter_mut() {
                if self.swapped[step] {
                    column.swap(step, step + 1);
                }
                let change = self.multipliers[step] * column[step];
                column[step + 1] -= change;
            }
        }
        let mut accumulators = try_filled(columns.len(), Complex::<f64>::zero())?;
        for row in (0..n).rev() {
            for (accumulator, column) in accumulators.iter_mut().zip(columns.iter()) {
                *accumulator = column[row];
            }
            let entries = &self.upper[row * n + row + 1..(row + 1) * n];
            for (offset, &entry) in entries.iter().enumerate() {
                for (accumulator, column) in accumulators.iter_mut().zip(columns.iter()) {
                    *accumulator -= entry * column[row + 1 + offset];
                }
            }
            let diagonal = self.upper[row * n + row];
            for (accumulator, column) in accumulators.iter().zip(columns.iter_mut()) {
                column[row] = divide(*accumulator, diagonal);
            }
        }
        Ok(())
    }
}

pub(super) enum GroupOutcome {
    Refined(Split, Vec<f64>),
    Dependent,
    SubproblemFailed,
}

fn random_unit(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
}

fn orthonormalize(basis: &mut Split) -> bool {
    let (rows, columns) = (basis.rows, basis.columns);
    for column in 0..columns {
        let norm_of = |basis: &Split, column: usize| -> f64 {
            (0..rows)
                .map(|row| {
                    basis.re[column * rows + row].powi(2) + basis.im[column * rows + row].powi(2)
                })
                .sum::<f64>()
                .sqrt()
        };
        let before = norm_of(basis, column);
        for _ in 0..2 {
            for earlier in 0..column {
                let (mut dot_re, mut dot_im) = (0.0, 0.0);
                for row in 0..rows {
                    let (er, ei) = (
                        basis.re[earlier * rows + row],
                        basis.im[earlier * rows + row],
                    );
                    let (cr, ci) = (basis.re[column * rows + row], basis.im[column * rows + row]);
                    dot_re += er * cr + ei * ci;
                    dot_im += er * ci - ei * cr;
                }
                for row in 0..rows {
                    let (er, ei) = (
                        basis.re[earlier * rows + row],
                        basis.im[earlier * rows + row],
                    );
                    basis.re[column * rows + row] -= dot_re * er - dot_im * ei;
                    basis.im[column * rows + row] -= dot_re * ei + dot_im * er;
                }
            }
        }
        let after = norm_of(basis, column);
        if after.is_nan() || after <= DEPENDENCE_RATIO * before || after == 0.0 {
            return false;
        }
        for row in 0..rows {
            basis.re[column * rows + row] /= after;
            basis.im[column * rows + row] /= after;
        }
    }
    true
}

fn ritz_order(values: &[Complex<f64>], order: &mut [usize]) {
    for (index, slot) in order.iter_mut().enumerate() {
        *slot = index;
    }
    order.sort_unstable_by(|&left, &right| {
        values[right]
            .imaginary()
            .partial_cmp(values[left].imaginary())
            .unwrap_or(core::cmp::Ordering::Equal)
            .then_with(|| {
                values[right]
                    .real()
                    .partial_cmp(values[left].real())
                    .unwrap_or(core::cmp::Ordering::Equal)
            })
            .then(left.cmp(&right))
    });
}

pub(super) fn refine_group(
    matrix: &[f64],
    frobenius_norm: f64,
    floor: f64,
    hessenberg: &Hessenberg,
    members: &[usize],
    upper_lambda: &[Complex<f64>],
    center: Complex<f64>,
) -> Result<GroupOutcome, EigensolveFaerError> {
    let n = hessenberg.dimension;
    let g = members.len();
    let lu = HessenbergLu::new(hessenberg, center, floor)?;
    let mut seed =
        0x9E37_79B9_7F4A_7C15u64 ^ (members[0] as u64).wrapping_mul(0xD1B5_4A32_D192_ED03);
    let mut basis = Split::zeros(n, g)?;
    for index in 0..n * g {
        basis.re[index] = random_unit(&mut seed);
        basis.im[index] = random_unit(&mut seed);
    }
    let mut member_lambda = try_filled(g, Complex::<f64>::zero())?;
    for (slot, &member) in members.iter().enumerate() {
        member_lambda[slot] = upper_lambda[member];
    }
    let mut order_members: Vec<usize> = try_filled(g, 0)?;
    ritz_order(&member_lambda, &mut order_members);
    let mut work = Split::zeros(n, g)?;
    let mut previous = f64::INFINITY;
    let mut result: Option<(Split, Vec<f64>)> = None;
    for iteration in 0..MAX_ITERATIONS {
        for (target, source) in [(&mut work.re, &basis.re), (&mut work.im, &basis.im)] {
            matmul(
                MatMut::from_column_major_slice_mut(target, n, g),
                Accum::Replace,
                MatRef::from_column_major_slice(&hessenberg.q, n, n).transpose(),
                MatRef::from_column_major_slice(source, n, g),
                1.0,
                Par::Seq,
            );
        }
        let mut columns: Vec<Vec<Complex<f64>>> = Vec::new();
        columns
            .try_reserve_exact(g)
            .map_err(|_| crate::allocation_error(g))?;
        for column in 0..g {
            let mut values = try_filled(n, Complex::<f64>::zero())?;
            for (row, value) in values.iter_mut().enumerate() {
                *value = Complex::new(work.re[column * n + row], work.im[column * n + row]);
            }
            columns.push(values);
        }
        lu.solve(&mut columns)?;
        for (column, values) in columns.iter().enumerate() {
            for (row, value) in values.iter().enumerate() {
                work.re[column * n + row] = *value.real();
                work.im[column * n + row] = *value.imaginary();
            }
        }
        for (target, source) in [(&mut basis.re, &work.re), (&mut basis.im, &work.im)] {
            matmul(
                MatMut::from_column_major_slice_mut(target, n, g),
                Accum::Replace,
                MatRef::from_column_major_slice(&hessenberg.q, n, n),
                MatRef::from_column_major_slice(source, n, g),
                1.0,
                Par::Seq,
            );
        }
        if !orthonormalize(&mut basis) {
            return Ok(GroupOutcome::Dependent);
        }
        let mut image = Split::zeros(n, g)?;
        real_product(matrix, n, &basis.re, g, &mut image.re);
        real_product(matrix, n, &basis.im, g, &mut image.im);
        let mut projected = try_filled(g * g, c64::new(0.0, 0.0))?;
        for q in 0..g {
            for p in 0..g {
                let (mut sum_re, mut sum_im) = (0.0, 0.0);
                for row in 0..n {
                    let (yr, yi) = (basis.re[p * n + row], -basis.im[p * n + row]);
                    let (ar, ai) = (image.re[q * n + row], image.im[q * n + row]);
                    sum_re += yr * ar - yi * ai;
                    sum_im += yr * ai + yi * ar;
                }
                projected[q * g + p] = c64::new(sum_re, sum_im);
            }
        }
        let mut ritz_values = try_filled(g, c64::new(0.0, 0.0))?;
        let mut ritz_vectors = try_filled(g * g, c64::new(0.0, 0.0))?;
        let params = Spec::default();
        let mut workspace = try_workspace(evd_scratch::<c64>(
            g,
            ComputeEigenvectors::No,
            ComputeEigenvectors::Yes,
            Par::Seq,
            params,
        ))?;
        if evd_cplx(
            MatRef::from_column_major_slice(&projected, g, g),
            DiagMut::from_slice_mut(&mut ritz_values),
            None,
            Some(MatMut::from_column_major_slice_mut(&mut ritz_vectors, g, g)),
            Par::Seq,
            MemStack::new(&mut workspace),
            params,
        )
        .is_err()
        {
            return Ok(GroupOutcome::SubproblemFailed);
        }
        let mut mu = try_filled(g, Complex::<f64>::zero())?;
        for (slot, value) in mu.iter_mut().zip(&ritz_values) {
            *slot = Complex::new(value.re, value.im);
        }
        let mut combined = Split::zeros(n, g)?;
        let mut combined_image = Split::zeros(n, g)?;
        for column in 0..g {
            for p in 0..g {
                let weight = ritz_vectors[column * g + p];
                for row in 0..n {
                    let (yr, yi) = (basis.re[p * n + row], basis.im[p * n + row]);
                    let (ar, ai) = (image.re[p * n + row], image.im[p * n + row]);
                    combined.re[column * n + row] += yr * weight.re - yi * weight.im;
                    combined.im[column * n + row] += yr * weight.im + yi * weight.re;
                    combined_image.re[column * n + row] += ar * weight.re - ai * weight.im;
                    combined_image.im[column * n + row] += ar * weight.im + ai * weight.re;
                }
            }
        }
        let mut ritz_sorted: Vec<usize> = try_filled(g, 0)?;
        ritz_order(&mu, &mut ritz_sorted);
        let mut assigned = Split::zeros(n, g)?;
        let mut ratios = try_filled(g, 0.0)?;
        for (position, &member_slot) in order_members.iter().enumerate() {
            let ritz = ritz_sorted[position];
            let mut norm_squared = 0.0;
            for row in 0..n {
                norm_squared +=
                    combined.re[ritz * n + row].powi(2) + combined.im[ritz * n + row].powi(2);
            }
            let norm = norm_squared.sqrt();
            let (lambda_re, lambda_im) = (
                *member_lambda[member_slot].real(),
                *member_lambda[member_slot].imaginary(),
            );
            let mut residual = 0.0;
            for row in 0..n {
                let at = ritz * n + row;
                let (xr, xi) = (combined.re[at], combined.im[at]);
                let er = combined_image.re[at] - (lambda_re * xr - lambda_im * xi);
                let ei = combined_image.im[at] - (lambda_re * xi + lambda_im * xr);
                residual += er * er + ei * ei;
                assigned.re[member_slot * n + row] = xr / norm;
                assigned.im[member_slot * n + row] = xi / norm;
            }
            ratios[member_slot] = residual.sqrt() / (frobenius_norm * norm);
        }
        let worst = ratios.iter().copied().fold(0.0, f64::max);
        result = Some((assigned, ratios));
        let stalled = iteration >= 1 && worst > 0.5 * previous;
        previous = worst;
        if worst <= TRIGGER || stalled || iteration + 1 == MAX_ITERATIONS {
            break;
        }
        if let Some((current, _)) = &result {
            basis.re.copy_from_slice(&current.re);
            basis.im.copy_from_slice(&current.im);
        }
    }
    match result {
        Some((assigned, ratios)) => Ok(GroupOutcome::Refined(assigned, ratios)),
        None => Ok(GroupOutcome::Dependent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orthonormalization_rejects_parallel_columns() {
        let mut basis = Split::zeros(3, 2).unwrap();
        basis.re.copy_from_slice(&[1.0, 2.0, 3.0, 2.0, 4.0, 6.0]);
        assert!(!orthonormalize(&mut basis));
    }
}
