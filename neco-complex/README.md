# neco-complex

[日本語](README-ja.md)

`neco-complex` provides complex scalar types for numerical linear algebra and signal processing.
The real and imaginary parts are stored as private fields, and construction accepts any component values.
Arithmetic follows the operations of the component type, and non-finite values and division by zero are handled as in the component type.

## Public API

- `Complex<T>`: a complex scalar that holds private components read through accessors
- `Complex::new(re, im)`: stores the given real and imaginary parts as they are
- `Complex::real`: returns a reference to the real part
- `Complex::imaginary`: returns a reference to the imaginary part
- `Complex::real_value`: returns the value of the real part
- `Complex::imaginary_value`: returns the value of the imaginary part
- `Complex::set_real`: replaces the real part
- `Complex::set_imaginary`: replaces the imaginary part
- `Complex::conjugate`: returns the complex conjugate
- `Complex::norm_squared`: returns the squared magnitude
- `Complex<f64>::from_real`: embeds a real scalar into the complex numbers
- `Complex<f32>::argument`: returns the argument (polar angle) in the `std` configuration
- `Complex<f64>::argument`: returns the argument (polar angle) in the `std` configuration
- `Complex<f64>::norm`: returns the magnitude (Euclidean norm) in the `std` configuration

## Runtime configuration

The standard library is used by default.
Disabling default features selects a `core`-only configuration that needs no dynamic memory allocation.

## License

MIT License.
