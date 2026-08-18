# neco-linear-exact

[日本語](README-ja.md)

This crate provides exact dense linear algebra. It supports rational values, normalized radical sums, and real algebraic values. Each matrix stores a validated shape and its elements in row-major order.

## Public API

- `ExactScalar`: fallible scalar operations
- `ExactLinearError`: operation failures
- `ExactMatrix<T>`: exact row-major matrix
- `ExactLinearSolution<T>`: system solution
- `determinant`: square-matrix determinant
- `rank`: matrix rank
- `kernel_basis`: kernel vectors
- `solve`: linear-system solution
- `project_vector_f64`: certified vector projection
- `project_matrix_f64`: certified matrix projection

Certified projections take an exact vector or an `ExactMatrix<T>` as input and provide these values:

- `CertifiedVectorProjection`: policy, per-element certificates, and the sum of their absolute-error bounds
- `CertifiedMatrixProjection`: policy, row-major per-element certificates, the projected `DenseMatrix<f64>`, and the sum of the absolute-error bounds

Elimination scans columns from left to right. It selects the first nonzero row at or below the pivot row. The resulting pivot sequence determines row exchanges, kernel vectors, and solution vectors.

## Runtime configuration

The default configuration uses the standard library. Disabling default features selects `no_std` with `alloc`.

## License

MIT License.
