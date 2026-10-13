# neco-generalized-eigen

[日本語](README-ja.md)

`neco-generalized-eigen` provides validated public types for generalized eigenvalue problems.
It can also be used in the `core + alloc` configuration.
Problem data, eigenpairs, mass-orthonormal eigenspaces, mass-metric projectors, convergence states, and spectral shifts can be exchanged with eigensolvers as validated values.

## Public API

- `GeneralizedEigenProblem`: dense stiffness and mass matrices with compatible shapes
- `GeneralizedEigenProblem::from_dense`: checked construction of a dense problem
- `GeneralizedEigenProblem::from_csr`: construction by checked conversion of CSR matrices to dense matrices
- `EigenResidual`: absolute and relative residuals
- `EigenResidual::meets_tolerance`: the absolute-or-relative residual convergence check
- `Eigenpair`: a validated eigenvalue, eigenvector, and residual
- `Eigenspace`: a mass-orthonormal basis belonging to one eigenvalue of the given problem
- `EigenProjector`: the mass-metric projection onto an eigenspace
- `ConvergenceStatus`: solver progress information, constructible only through validated constructors
- `EigenShift`: the spectral shift that a solver records in its result, a finite `Complex<f64>` value
- `GeneralizedEigenError`: validation and linear-algebra failures

`Eigenspace::new` recomputes the residual of each basis vector from the same input problem and the given eigenvalue.
An eigenspace can be constructed only when the mass norm of each basis vector, the mutual orthogonality of the basis, and the origin of each eigenpair (its residual matches the value recomputed from the input problem) pass validation.

## Mode counts

The convergence state distinguishes the requested count, the returned count, and the count that satisfies the tolerances.

```text
requested_modes: count requested from the solver
returned_modes: count returned as eigenspaces
converged_modes: count among the returned modes whose residual satisfies the tolerances
```

The relative residual is the absolute residual divided by the largest of three values.

- The norm of the product of the stiffness matrix and the eigenvector
- The norm of the product of the mass matrix and the eigenvector, multiplied by the eigenvalue
- The smallest positive normal `f64` value (`f64::MIN_POSITIVE`)

The solver returns the complete eigenspace of each exactly repeated eigenvalue.
The returned and converged counts can therefore exceed the requested count.
The converged count is at most the returned count, and the two counts are equal in a converged result.

## Dependencies

- `neco-linear-types`: vector and linear operator types
- `neco-linear-dense`: dense problem matrices
- `neco-sparse`: conversion of problem matrices in CSR format
- `neco-complex`: complex numbers that represent spectral shifts

## Runtime configuration

The standard library is used by default.
Disabling default features selects the `core + alloc` configuration, which assumes only dynamic memory allocation.

## License

MIT License.
