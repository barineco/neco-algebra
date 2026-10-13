use crate::convert::{
    from_faer_values, to_faer_values, validate_finite_complex, validate_square_complex,
};
use crate::{try_filled, try_workspace, EigensolveFaerError};
use faer::dyn_stack::MemStack;
use faer::linalg::lu::partial_pivoting::factor::{lu_in_place, lu_in_place_scratch};
use faer::linalg::lu::partial_pivoting::solve::{solve_in_place, solve_in_place_scratch};
use faer::perm::PermRef;
use faer::{c64, MatMut, MatRef, Par, Spec};
use neco_complex::Complex;
use neco_linear_dense::DenseMatrix;
use neco_linear_types::Shape;

const FACTOR: &str = "ComplexLu::factor";
const SOLVE: &str = "ComplexLu::solve";

#[derive(Clone, Debug)]
pub struct ComplexLu {
    dimension: usize,
    packed: Vec<c64>,
    permutation: Vec<usize>,
    inverse_permutation: Vec<usize>,
}

impl ComplexLu {
    pub fn factor(matrix: &DenseMatrix<Complex<f64>>) -> Result<Self, EigensolveFaerError> {
        let dimension = validate_square_complex(matrix, FACTOR)?;
        let mut packed = to_faer_values(matrix.values())?;
        let mut permutation = try_filled(dimension, 0usize)?;
        let mut inverse_permutation = try_filled(dimension, 0usize)?;
        let params = Spec::default();
        let mut workspace = try_workspace(lu_in_place_scratch::<usize, c64>(
            dimension,
            dimension,
            Par::Seq,
            params,
        ))?;
        lu_in_place(
            MatMut::from_column_major_slice_mut(&mut packed, dimension, dimension),
            &mut permutation,
            &mut inverse_permutation,
            Par::Seq,
            MemStack::new(&mut workspace),
            params,
        );
        for index in 0..dimension {
            let pivot = packed[index * dimension + index];
            if !pivot.re.is_finite()
                || !pivot.im.is_finite()
                || (pivot.re == 0.0 && pivot.im == 0.0)
            {
                return Err(EigensolveFaerError::SingularPivot {
                    operation: FACTOR,
                    index,
                    magnitude: pivot.re.hypot(pivot.im),
                });
            }
        }
        Ok(Self {
            dimension,
            packed,
            permutation,
            inverse_permutation,
        })
    }

    pub fn solve(
        &self,
        rhs: &DenseMatrix<Complex<f64>>,
    ) -> Result<DenseMatrix<Complex<f64>>, EigensolveFaerError> {
        if rhs.rows() != self.dimension {
            return Err(EigensolveFaerError::RightHandSide {
                operation: SOLVE,
                expected: self.dimension,
                actual: rhs.rows(),
            });
        }
        validate_finite_complex(rhs.values(), SOLVE)?;
        if rhs.columns() == 0 {
            return Ok(DenseMatrix::from_column_major(
                Shape::new(self.dimension, 0),
                Vec::new(),
            )?);
        }
        let mut solution = to_faer_values(rhs.values())?;
        let mut workspace = try_workspace(solve_in_place_scratch::<usize, c64>(
            self.dimension,
            rhs.columns(),
            Par::Seq,
        ))?;
        let packed = MatRef::from_column_major_slice(&self.packed, self.dimension, self.dimension);
        solve_in_place(
            packed,
            packed,
            PermRef::new_checked(&self.permutation, &self.inverse_permutation, self.dimension),
            MatMut::from_column_major_slice_mut(&mut solution, self.dimension, rhs.columns()),
            Par::Seq,
            MemStack::new(&mut workspace),
        );
        let values = from_faer_values(&solution)?;
        if !values
            .iter()
            .all(|value| value.real().is_finite() && value.imaginary().is_finite())
        {
            return Err(EigensolveFaerError::NonFiniteResult { operation: SOLVE });
        }
        Ok(DenseMatrix::from_column_major(
            Shape::new(self.dimension, rhs.columns()),
            values,
        )?)
    }
}
