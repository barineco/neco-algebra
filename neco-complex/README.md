# neco-complex

[日本語](README-ja.md)

`neco-complex` provides complex scalar types for numerical linear algebra and signal processing.

Values store private real and imaginary components, and construction accepts any component values. Arithmetic follows the component type, including its handling of non-finite values and division by zero.

## Public API

- `Complex<T>`: a complex scalar with private components and plain accessor methods
- `Complex::new(re, im)`: stores the supplied components
- `Complex::real`: observes the real component by reference
- `Complex::imaginary`: observes the imaginary component by reference
- `Complex::real_value`: returns the real component value
- `Complex::imaginary_value`: returns the imaginary component value
- `Complex::set_real`: replaces the real component
- `Complex::set_imaginary`: replaces the imaginary component
- `Complex::conjugate`: returns the conjugate scalar
- `Complex::norm_squared`: returns the squared magnitude
- `Complex<f64>::from_real`: embeds a real scalar
- `Complex<f32>::argument`: returns the polar angle in the `std` configuration
- `Complex<f64>::argument`: returns the polar angle in the `std` configuration
- `Complex<f64>::norm`: returns the Euclidean magnitude in the `std` configuration

## Runtime configuration

The default configuration uses the standard library. Disabling default features selects `core` without allocation.

## License

MIT License.
