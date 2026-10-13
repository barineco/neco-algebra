# neco-eigensolve-faer

[日本語](README-ja.md)

`neco-eigensolve-faer` is an adapter crate that uses `faer` to solve eigenvalue problems and complex linear systems, and to compute condition numbers of complex matrices.

- Solving real symmetric generalized eigenvalue problems with positive-definite mass matrices
- Solving real nonsymmetric eigenvalue problems
- Solving complex dense linear systems
- Computing 2-norm condition numbers of complex matrices

The argument and result types are defined by the linear algebra and eigenvalue crates in this repository.
Failures specific to this crate are returned as `EigensolveFaerError` values.

## API

```text
solve_symmetric_f64(&GeneralizedEigenProblem, EigensolveConfig)
  -> Result<EigensolveResult, EigensolveFaerError>

solve_request_symmetric_f64(EigensolveRequest<R>)
  -> Result<EigensolveResult<R>, EigensolveFaerError>
```

```text
approximate_mass_solve(&DenseMatrix<f64>, &Vector<f64>)
  -> Result<Vector<f64>, EigensolveFaerError>
```

```text
solve_nonsymmetric_f64(&DenseMatrix<f64>)
  -> Result<NonsymmetricEigenDecomposition, EigensolveFaerError>

NonsymmetricEigenDecomposition::eigenvalues(&self) -> &[Complex<f64>]
NonsymmetricEigenDecomposition::eigenvectors(&self) -> &DenseMatrix<Complex<f64>>

ComplexLu::factor(&DenseMatrix<Complex<f64>>)
  -> Result<ComplexLu, EigensolveFaerError>

ComplexLu::solve(&self, &DenseMatrix<Complex<f64>>)
  -> Result<DenseMatrix<Complex<f64>>, EigensolveFaerError>

condition_number_2(&DenseMatrix<Complex<f64>>)
  -> Result<f64, EigensolveFaerError>
```

### Results

- `EigensolveResult<R>`: mass-normalized eigenvectors, residuals computed from the input problem, a zero spectral shift, and the projection reference moved from the request (an arbitrary value of the caller's type `R`)
- `EigensolveFaerError`: failures from matrices, the external solver, and lower crates
- `approximate_mass_solve`: an approximate solution of the linear system $M z = b$ whose coefficient matrix is the mass matrix $M$, computed through the same Cholesky path as the solver; callers use it only as a correction input for an independently certified forward-error bound, never as the bound itself

The conditions on the input matrices differ between the stiffness matrix and the mass matrix:

- Stiffness matrix: every element is finite, and the matrix is symmetric; an input that violates this is returned as `InvalidStiffnessMatrix`
- Mass matrix: every element is finite, and the matrix is symmetric and positive definite; an input that contains a non-finite element or is not symmetric is returned as `InvalidMassMatrix`
  - An input whose Cholesky diagonal reaches 0 or below is returned as `NonPositiveMassMatrix`

Exactly repeated eigenvalues are returned as a complete eigenspace.
The API that takes a request moves its projection reference unchanged into the result.

### Nonsymmetric eigenvalues, complex LU decomposition, and condition numbers

- Eigenvalues: returns every eigenvalue as a complex number
- Eigenvectors: returns right eigenvectors as the columns of a column-major complex matrix
- Normalization: unit 2-norm; among the components whose modulus is at least $(1 - 10^{-12})$ times the largest modulus, the one with the smallest index is a positive real number
- Conjugate pairs: the eigenvector for an eigenvalue with a negative imaginary part is the exact conjugate of the eigenvector for the eigenvalue with the positive imaginary part
- Order: descending imaginary part, then descending real part; eigenvalues equal in both parts keep the order returned by the underlying solver

The result holds only the eigenvalues and the eigenvectors.
The contract below bounds the residual of every eigenpair, and callers can recompute the residual from their own matrix.

The LU decomposition of a complex matrix is computed with `ComplexLu::factor`, and linear systems are solved with `solve` on the factorization.
The condition number is computed with `condition_number_2`.

- Factorization: holds an LU decomposition with partial pivoting as a value that serves any number of right-hand sides
- Solution: returns an n by m solution for an n by m right-hand side
- Pivot check: after factorization, examines the diagonal of U and returns, as a failure, the zero or non-finite entry with the smallest index, together with the operation name, the index, and the pivot magnitude
- Right-hand side with zero columns: returns a solution with zero columns
- Condition number: returns the ratio of the largest to the smallest singular value of a square, finite, nonzero matrix; returns positive infinity when the smallest singular value is zero
- Acceptance: callers judge whether a condition number is acceptable

Every underlying computation in these operations runs sequentially, so results are independent of the global parallelism setting of faer.
When allocating scratch, result, or ordering storage fails, the failure is returned as a value.
On the same machine with the same build, the same input gives a result with the same bits, and this guarantee is limited to that machine and build.

The eigendecomposition and the condition number divide the input matrix by the largest power of two at or below the largest element magnitude before calling the underlying solver.
The eigenvalues are restored to the original scale, and the condition number keeps its value because it is a ratio. The eigenvectors are computed from the scaled matrix and returned after normalization.
The accuracy of the eigendecomposition is determined by the backward error relative to the scale of the largest element.
In the scaled matrix, elements below about $2^{-1022}$ times the largest element become subnormal or zero.
The LU decomposition calls the underlying solver at the original scale.

**Rebuilding eigenpairs with a large backward error**

- Contract: every returned eigenpair has a backward error of at most 1e-12; the backward error is the residual norm divided by the product of the Frobenius norm of the matrix and the norm of the eigenvector
- Check: the backward error of every eigenpair is computed after the underlying decomposition
- Rebuild condition: a complex eigenpair whose backward error exceeds 16 times the machine epsilon is grouped with the eigenvalues within a relative distance of 1e-6 and rebuilt by shift-and-invert subspace iteration on the Hessenberg form and the Rayleigh-Ritz method
- Limits: at most 8 iterations per group; the rebuilt eigenvectors of a group must have a 2-norm condition number of at most 1e8
- Replacement: a rebuilt eigenvector replaces the original only when its backward error is smaller than the original
- Groups near the real axis: a group fails before any rebuild when the mean of its eigenvalues has an imaginary part of at most 1e-6 times the modulus of that mean
- Unchanged output: for a matrix with no eigenpair to rebuild, the result matches, bit for bit, the output of the underlying decomposition with the normalization and ordering rules above

The kinds of failure are distinguished by the variants of `EigensolveFaerError`:

```text
InvalidMatrix { operation, reason }
  non-square, empty, or non-finite input; a zero matrix passed to the condition number computation.
  operation names the failing function.
SingularPivot { operation, index, magnitude }
  a zero or non-finite pivot in the factorization, with its magnitude.
RightHandSide { operation, expected, actual }
  the right-hand side row count differs from the dimension of the factorized matrix.
Linear(AllocationFailure | CapacityOverflow)
  allocating scratch, result, or ordering storage failed.
EigenvectorRefinement { operation, index, residual, kind }
  ResidualExceeded: an eigenpair whose backward error still exceeds 1e-12 after the rebuild.
  RealEigenvalue: a real eigenvalue whose backward error exceeds 16 times the machine epsilon.
  LinearlyDependent: a group whose eigenvectors after the rebuild have a condition number above 1e8, or a group in which the orthonormalization of subspace iteration detected linear dependence.
  NearRealAxis: a group close to the real axis, reported before any rebuild.
  SubproblemFailed: the small eigenvalue or singular value problem failed to converge.
  index is the position in the returned order.
NonFiniteResult { operation }
  an eigenvalue, an eigenvector, or a solution has a non-finite value.
ExternalSolver { operation, reason }
  the eigendecomposition or singular value decomposition failed to converge, or returned a zero eigenvector or a complex eigenvalue without its conjugate partner.
```

Physical interpretation, such as overdamping or stability, is left to the caller.

### Tridiagonal QR algorithm for all eigenvectors

The adapter calls the public function `faer::linalg::evd::self_adjoint_evd` once to compute every eigenvalue and the corresponding eigenvector columns.
`recursion_threshold` is set to n + 1, computed with checked addition, where n is the matrix dimension.
This setting selects the tridiagonal QR algorithm that produces eigenvectors.
This call runs under the global parallelism setting of faer.
Whether the mass matrix is exactly the identity or a general positive-definite matrix, the eigendecomposition is called only this once.

When the mass matrix is a general positive-definite matrix, the adapter uses the Cholesky reduction to a standard problem, the column transformation by the inverse triangular matrix, and the mass inner product over all elements.
When the mass matrix is exactly the identity, it uses the stiffness matrix directly and obtains finite unnormalized columns without storing or multiplying by the inverse of the identity matrix.
For identity mass, nonzero finite components keep their bits before normalization, and only signed zeros become positive zero.
Normalization, the sign choice by the component of largest modulus, and the validation of public residuals and eigenspaces still use the original input problem.

The public residual tolerances determine the convergence status after decomposition.
This crate returns numerical solutions and residuals, and certified forward-error bounds are computed by `neco-eigensolve-certified`.
`EigensolveConfig` must be given a positive `max_iterations`.
However, the QR algorithm uses its own internal iteration limit.
The returned iteration count of 1 stands for the one decomposition the adapter performs, separate from the internal QR iteration count and its limit.

The eigenvalue and eigenvector storage is reserved, after checking that its element count fits in `usize` and its byte size is at most `isize::MAX`, in a way that receives allocation failure as a value.
For both this storage and the scratch storage, capacity overflow is returned as `Linear(CapacityOverflow)`, and allocation refusal as `Linear(AllocationFailure)`.
The count requested for scratch allocation is a byte size.
For an empty matrix, an empty eigenvalue sequence and an empty eigenvector sequence are returned before scratch storage is requested.
Other failures returned by the decomposition are returned as `ExternalSolver`.
These guarantees apply to the eigenvalue and eigenvector storage and to the scratch storage.
Input conversion and general mass conversion allocate `faer` matrices.
An allocation failure there and a panic inside an external library stop the operation through Rust's standard handling (a panic or an abort).

## Runtime configuration

This adapter uses the standard library feature of `faer` and therefore requires the standard library.

## License

MIT License.
