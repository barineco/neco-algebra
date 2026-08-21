# neco-linear-dense

[日本語](README-ja.md)

`neco-linear-dense` provides dense matrices for numerical linear algebra.

Each matrix stores a shape and exactly the corresponding number of elements in column-major order. Construction validates storage length, element-count capacity, representable storage length, and allocation. Matrix-vector multiplication for `f64` values validates the input-vector length.

## Public API

- `DenseMatrix<T>`: dense matrix with private shape and storage
- `from_column_major`: constructs a matrix from column-major values
- `from_row_major`: converts row-major values to column-major storage
- `try_zeros`: constructs storage filled with one value
- `value`: observes an element using validated row and column indices
- `LinearOperator<f64>`: applies a matrix to a vector

## Runtime configuration

The default configuration uses the standard library. Disabling default features selects `no_std` with `alloc`.

## License

MIT License.
