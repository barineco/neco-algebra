# neco-linear-exact

[日本語](README-ja.md)

`neco-linear-exact` is a crate that provides exact linear algebra on dense matrices.
It handles rationals, normalized formal sums of radicals, and real algebraic numbers.
Each matrix holds a validated shape and its elements stored in row-major order.

## Public API

- `ExactScalar`: scalar operations that return failures as values
- `ExactLinearError`: operation failures
- `ExactMatrix<T>`: an exact row-major matrix with validated shape and storage length
- `ExactLinearSolution<T>`: three states, a unique solution, an affine solution with a kernel basis, or no solution
- `determinant`: the determinant of a square matrix
- `rank`: the rank of a matrix
- `kernel_basis`: the basis vectors of the kernel
- `solve`: the solution of a linear system
- `CertifiedMatrixInput`: an input that pairs a numerical matrix with a certified expression bundle (`CertifiedF64Bundle`) and row-major expression identifiers
- `CertifiedMatrixEntry`: a value that holds, for one matrix location, the expression identifier and the certified value
- `MassNormalizationReference`: a reference that pairs a mass matrix input with how to treat it, either as the identity matrix or as a positive-definite matrix factored by Cholesky decomposition
- `CertifiedGeneralizedEigenInput`: an input that combines the stiffness matrix input with the mass matrix input and how to treat it
- `project_vector_f64`: certified projection of a vector
- `project_matrix_f64`: certified projection of a matrix

## Certified matrix inputs

`CertifiedMatrixInput::from_product_projection` constructs a matrix input by comparing a dense matrix with one certified expression bundle (`CertifiedF64Bundle`) contained in an exact computation result. The comparison uses the provenance carried by both the computation result and the expression bundle.

- `ExactComputationProduct`: the exact computation result and its provenance
- `CertifiedF64Bundle`: the certified expressions and their provenance

The items compared for equality are values held by both the computation result and the expression bundle.

- The shape and the row-major expression identifiers
- Owner, consumer, requested precision, and implementation source, as defined by `neco-expr`
- The opaque computation result identifier
- The `f64` bit patterns

The constructed input keeps the compared `CertifiedF64Bundle` as its provenance source.
The value and provenance of each entry are read with `entry`, and validation failures are received as `NecoFailure`.

- `entry(row, column)`: the matrix location, expression identifier, consumer, certified value, enclosure, and absolute error bound of the projection
- `NecoFailure`: reports the kind of matrix input that failed validation (`MatrixInputRole`: a general matrix, the stiffness matrix, the mass matrix, or the reference to the mass matrix and how to treat it); a failure caused by an entry also reports its row and column

`CertifiedGeneralizedEigenInput` is an input that combines the stiffness matrix input with the required `MassNormalizationReference`.

- Treatment of the mass matrix: `Identity` treats it as the identity matrix, and `PositiveDefiniteCholesky` factors it as a positive-definite matrix by Cholesky decomposition
- `mass()`: returns the mass matrix input and how to treat it, for the eigensolver to read

## Matrix observation and certified projections

`ExactMatrix<T>` gives access to its shape, row count, column count, the value at a given coordinate, and all elements in row-major order.
Certified projections compute, from each element of an exact vector or matrix, a finite `f64` value and an interval that contains the exact value. The projection results are returned as one type for vectors and one for matrices.

- `CertifiedVectorProjection`: the projection policy, the projected vector, the certified value of each element, and the sum of the absolute error bounds
- `CertifiedMatrixProjection`: the projection policy, the projected matrix, the certified value of each element in row-major order, and the sum of the absolute error bounds

## Gaussian elimination

Gaussian elimination scans the columns from left to right and selects, as the next pivot row, the first nonzero row at or below the current pivot row.
This deterministic selection determines the row exchanges, the kernel basis, and the solution vectors.

## Runtime configuration

The standard library is used by default.
Disabling default features selects the `core + alloc` configuration, which assumes only dynamic memory allocation.

## License

MIT License.
