# neco-linear-dense

[日本語](README-ja.md)

`neco-linear-dense` provides dense matrices for numerical linear algebra.
Each matrix holds exactly as many elements as its shape requires, in column-major order.
Construction checks that rows × columns fits in `usize`, that the element count equals that product, and that the element count is at most `isize::MAX`, and confirms that memory allocation succeeds.
Matrix-vector multiplication for `f64` also validates the length of the input vector.

## Public API

- `DenseMatrix<T>`: a dense matrix that holds its shape and elements in private fields
- `from_column_major`: constructs a matrix from column-major elements
- `from_row_major`: constructs a matrix by converting row-major elements to column-major storage order
- `try_zeros`: constructs a matrix with every element initialized to one value
- `value`: references an element by validated row and column indices
- `LinearOperator<f64>`: computes the product of the matrix and a vector

## Runtime configuration

The standard library is used by default.
Disabling default features selects the `core + alloc` configuration, which assumes only dynamic memory allocation.

## License

MIT License.
