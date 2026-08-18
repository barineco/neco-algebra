# neco-generalized-eigen

[日本語](README-ja.md)

`neco-generalized-eigen` provides validated public types for generalized eigenvalue problems in `no_std + alloc` environments. Numerical solvers use its problem data, eigenpairs, mass-orthonormal eigenspaces, mass-metric projectors, convergence states, and complex shifts as their public values.

## Public API

- `GeneralizedEigenProblem`: compatible dense stiffness and mass matrices
- `GeneralizedEigenProblem::from_dense`: checked dense-problem construction
- `GeneralizedEigenProblem::from_csr`: checked CSR-to-dense conversion
- `EigenResidual`: absolute and relative residual
- `Eigenpair`: validated eigenvalue, vector, and residual
- `Eigenspace`: a mass-orthonormal basis for one eigenvalue of the given problem
- `EigenProjector`: mass-metric eigenspace projection
- `ConvergenceStatus`: solver progress metadata, constructible only through validated constructors
- `EigenShift`: finite `Complex<f64>` spectral shift
- `GeneralizedEigenError`: validation and linear-algebra failures

## Mode counts

```text
requested_modes: count requested from the solver
returned_modes: count in returned eigenspaces
converged_modes: returned count satisfying tolerances

converged_modes <= returned_modes
Converged: converged_modes == returned_modes
```

The relative residual divides the absolute residual by the largest of the stiffness-product norm, the eigenvalue-scaled mass-product norm, and the minimum positive `f64` value.

The solver returns the complete eigenspace of each exactly repeated eigenvalue. Returned and converged counts can exceed the requested count.

## Dependencies

- `neco-linear-types`: vector and linear operator types
- `neco-linear-dense`: dense problem matrices
- `neco-sparse`: CSR problem-matrix conversion
- `neco-complex`: complex spectral shifts

## Runtime configuration

The default configuration uses the standard library. Disabling default features selects `no_std` with `alloc`.

## License

MIT License.
