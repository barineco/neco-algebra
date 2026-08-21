# neco-eigensolve-faer

[日本語](README-ja.md)

`neco-eigensolve-faer` solves real symmetric generalized eigenvalue problems with positive-definite mass matrices. Problem, configuration, request, and result types come from the sibling linear algebra crates. This crate defines its own failure type, `EigensolveFaerError`.

## API

```text
solve_symmetric_f64(&GeneralizedEigenProblem, EigensolveConfig)
  -> Result<EigensolveResult, EigensolveFaerError>

solve_request_symmetric_f64(EigensolveRequest<R>)
  -> Result<EigensolveResult<R>, EigensolveFaerError>
```

The solver returns mass-normalized eigenvectors and residuals computed from the input problem. Stiffness and mass matrices must be finite and symmetric, and the mass matrix must be positive definite.

Exactly repeated eigenvalues return a complete eigenspace, and the spectral shift is zero. The request form moves its projection reference into the result. Invalid matrices and solver failures return `EigensolveFaerError`.

## Runtime configuration

This adapter requires the standard library because it builds on `faer`.

## License

MIT License.
