# neco-bigint

[日本語](README-ja.md)

`neco-bigint` is a crate that provides arbitrary-precision arithmetic for exact computation.
Besides natural numbers, integers, and reduced rationals, it handles dyadic rationals and validated enclosures of them.
Operations whose memory use can grow return capacity overflows and allocation failures to the caller as `Result` values.

## Provided operations

Operations are provided for each of four kinds of numbers.

- Natural numbers: addition, subtraction, multiplication, shifts, division, powers, greatest common divisors, and least common multiples
- Integers: signed arithmetic, Euclidean division, powers, and the extended Euclidean algorithm
- Rationals: reduction with evidence, arithmetic, integer powers, floor and ceiling, and rounding to dyadic rationals
- Dyadic rationals: arithmetic on normal forms, exact conversion to and from finite floating-point values, ties-to-even rounding, and enclosures

### Validated values

The types for natural numbers, integers, reduced rationals, and dyadic rationals hide their internal representations, and only validated normal-form values can be constructed.
For rationals, the input before reduction and the validated result are represented by separate types and operations.

- `RawRational`: holds a numerator and denominator before reduction; a zero denominator can also be held
- `RawRational::reduce`: rejects a zero denominator, and on success produces a `RationalReduction`. This value holds the normal-form rational, and also holds the input and the greatest common divisor as evidence that the reduction is correct
- `BigintError::ZeroDenominator`: reports that the denominator was zero during reduction

### Stored forms

- Rational: $ -\tfrac{6}{8} $ is reduced and stored as $ -\tfrac{3}{4} $, with a positive denominator and coprime numerator and denominator
- Dyadic rational: stored as an integer $ m $ and a nonnegative exponent $ k $ in the form $ m / 2^{k} $, which represents values such as $ 0.625 = 5 / 2^{3} $ exactly
- Enclosure: holds dyadic endpoints with validated order, as in $ [\tfrac{1}{2}, \tfrac{3}{4}] $

### How failures are returned

Operations whose memory use can grow return their results in this type:

```text
Result<_, BigintError>
```

## Usage

This example reduces a rational and then computes a negative integer power.

```rust
use neco_bigint::{BigInt, BigUint, RawRational};

fn main() -> Result<(), neco_bigint::BigintError> {
    let numerator = BigInt::try_from(-6_i32)?;
    let denominator = BigUint::try_from(8_u32)?;
    let raw = RawRational::new(numerator, denominator);
    let reduction = raw.reduce()?;
    let reduced = reduction.reduced();

    let expected_numerator = BigInt::try_from(-3_i32)?;
    assert_eq!(reduced.numerator(), &expected_numerator);
    let expected_denominator = BigUint::try_from(4_u32)?;
    assert_eq!(reduced.denominator(), &expected_denominator);

    let reciprocal_square = reduced.pow_i32(-2)?;
    let expected_numerator = BigInt::try_from(16_i32)?;
    assert_eq!(reciprocal_square.numerator(), &expected_numerator);
    let expected_denominator = BigUint::try_from(9_u32)?;
    assert_eq!(reciprocal_square.denominator(), &expected_denominator);
    Ok(())
}
```

A dyadic rational holds an integer $ m $ and the nonnegative exponent $ k $ of the denominator $ 2^{k} $, and represents the value $ m / 2^{k} $.
Conversion from a finite floating-point value is lossless, and conversion back to a floating-point value uses ties-to-even rounding.

```rust
use neco_bigint::{Dyadic, DyadicEnclosure};

fn main() -> Result<(), neco_bigint::BigintError> {
    let lower = Dyadic::from_f64_exact(0.5)?;
    let upper = Dyadic::from_f64_exact(0.75)?;
    let enclosure = DyadicEnclosure::new(lower, upper)?;

    let midpoint = enclosure.midpoint()?;
    let rounded = midpoint.round_to_f64_ties_even()?;
    assert_eq!(rounded, 0.625);
    let target = Dyadic::from_f64_exact(0.625)?;
    assert!(enclosure.contains_dyadic(&target));
    Ok(())
}
```

## Public API

### Public types

- `BigUint`: a normalized arbitrary-precision natural number stored as little-endian `u32` limbs
- `BigInt`: an integer represented as a sign and magnitude, with a single representation of zero
- `Sign`: the sign of an integer
  - `Negative`: negative
  - `Zero`: zero
  - `Positive`: positive
- `ExtendedGcd`: the greatest common divisor and two Bézout coefficients
- `RawRational`: a numerator and denominator before reduction
- `RationalReduction`: the input of a reduction, the greatest common divisor, and the reduced result
- `ReducedRational`: a rational with a positive denominator and coprime numerator and denominator
- `Dyadic`: a normalized number of the form of an integer divided by a power of 2
- `DyadicEnclosure`: an inclusive interval with validated endpoint order
- `BigintError`: a failure returned by checked operations

### Conversion from primitive integers

- `BigUint`: construction from unsigned primitive integers
- `BigInt`: construction from signed and unsigned primitive integers
- `TryFrom`: checked construction

### Cloning

An arbitrary-precision value allocates memory even when it is cloned.
Every type that owns a value therefore provides `try_clone`, which returns allocation failures.

### Rational reduction

- `RawRational::reduce`: validates the input and reduces it
- `RationalReduction`: holds the input of the reduction and the greatest common divisor together with the reduced result

## Failures

`BigintError` distinguishes ten kinds of failure:

- `CapacityOverflow`: a limb count, bit count, shift amount, or iteration count exceeded the capacity limit checked before the operation
- `AllocationFailure { requested_limbs }`: allocating storage failed; holds the required total limb count
- `UnsignedUnderflow`: subtraction of natural numbers would produce a negative result
- `DivisionByZero`: the divisor is zero
- `NonExactDivision`: exact division left a nonzero remainder
- `ZeroDenominator`: the rational to be reduced has a zero denominator
- `NonFiniteFloat`: the input of a conversion to a dyadic rational is infinity or NaN
- `FloatOutOfRange`: the magnitude of a dyadic rational exceeded the largest finite `f64` value
- `InvalidInterval`: the endpoints of an enclosure are reversed
- `ExponentOverflow`: the exponent of a dyadic rational exceeded its limit
  - `required`: the exponent required by the operation
  - `maximum`: the largest supported `u32` exponent

Operations whose memory use grows estimate the required total limb count while checking for overflow and confirm that it stays within `BigUint::MAX_LIMBS`.
They then allocate storage in a way that receives allocation failure as a value.
Capacity overflows and allocation failures are returned as ordinary `Result` values both with and without the standard library.

## Runtime configuration

The standard library is used by default.
Disabling default features selects the `core + alloc` configuration, which assumes only dynamic memory allocation.

```toml
[dependencies]
neco-bigint = { version = "0.1", default-features = false }
```

In the configuration that uses the standard library, `BigintError` implements `std::error::Error`.
Numeric behavior and failure kinds are the same in both configurations.

## License

MIT License.
