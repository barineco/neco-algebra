# neco-linear-exact

[日本語](README-ja.md)

This crate provides exact dense linear algebra. It supports rational values, normalized radical sums, and real algebraic values. Each matrix stores a validated shape and its elements in row-major order.

## Public API

- `ExactScalar`: fallible scalar operations
- `ExactLinearError`: operation failures
- `ExactMatrix<T>`: exact row-major matrix with validated shape and storage length
- `ExactLinearSolution<T>`: unique solution, affine solution with a kernel basis, or inconsistent system
- `determinant`: square-matrix determinant
- `rank`: matrix rank
- `kernel_basis`: kernel vectors
- `solve`: linear-system solution
- `project_vector_f64`: certified vector projection
- `project_matrix_f64`: certified matrix projection

The matrix type provides shape, row count, column count, indexed value access, and row-major value access. Certified projections take an exact vector or matrix as input and provide these values:

- `CertifiedVectorProjection`: policy, projected vector, per-element certificates, and the sum of their absolute-error bounds
- `CertifiedMatrixProjection`: policy, projected matrix, row-major per-element certificates, and the sum of their absolute-error bounds

Elimination scans columns from left to right. It selects the first nonzero row at or below the pivot row. The resulting pivot sequence determines row exchanges, kernel vectors, and solution vectors.

## Runtime configuration

The default configuration uses the standard library. Disabling default features selects `no_std` with `alloc`.

## License

MIT License.
