# neco-sparse

[日本語](README-ja.md)

`neco-sparse` is a sparse matrix crate for numerical linear algebra that can also be used without the standard library.
It converts matrices in coordinate format (COO) into compressed sparse row format (CSR).
The conversion sorts the coordinates and merges values at the same coordinate into one.

## Public API

- `CooMatrix<T>`: holds coordinates within the shape and their values in any order; duplicate coordinates are allowed
- `CooMatrix::to_csr`: sorts stably in row and column order and adds the values at duplicate coordinates
- `CsrMatrix<T>`: holds row offsets, column indices, and values in validated form; the row offsets are an array of length rows + 1 that gives where each row starts in the value array
- `CsrMatrix::from_parts`: constructs a matrix after checking the invariants of row offsets, column indices, and values
- `CsrMatrix::row`: borrows one row as `CsrRow`
- `CsrRow`: reads column indices, values, and their pairs
- `LinearOperator<f64>`: computes the product of a CSR matrix and a vector, checking the input length

The only direct dependency is `neco-linear-types`.
Conversion from a CSR matrix to a dense matrix is performed by `GeneralizedEigenProblem::from_csr` in `neco-generalized-eigen`.

## Runtime configuration

The standard library is used by default.
Disabling default features selects the `core + alloc` configuration, which assumes only dynamic memory allocation.

## License

MIT License.
