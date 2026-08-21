# neco-bigint

[日本語](README-ja.md)

`neco-bigint` provides arbitrary-precision arithmetic for exact computation. It covers natural numbers, integers, reduced rationals, dyadic rationals, and validated dyadic enclosures. Operations that can grow storage return capacity and allocation failures as `Result` values.

## Features

Operations are divided into four groups:

- Natural numbers: addition, subtraction, multiplication, shifts, division, powers, greatest common divisors, and least common multiples
- Integers: signed arithmetic, Euclidean division, powers, and extended greatest common divisors
- Rationals: reduction with evidence, arithmetic, integer powers, floor and ceiling, and dyadic rounding
- Dyadic rationals: arithmetic on normal forms, exact finite floating-point conversion, ties-to-even rounding, and enclosures

Natural numbers, integers, reduced rationals, and dyadic rationals hide their representations and can be constructed only in validated normal forms. Rational construction separates the unreduced input from the validated result.

- `RawRational`: preserves an unreduced numerator and denominator, including a zero denominator
- `RawRational::reduce`: rejects a zero denominator or returns a normal-form rational with reduction evidence
- `BigintError::ZeroDenominator`: reports a zero denominator during reduction

The stored values have the following forms:

- Rational: `-6/8` reduces to `-3/4`, with a positive denominator and coprime parts
- Dyadic rational: `m * 2^e`, such as the exact value `0.625 = 5 * 2^(-3)`
- Dyadic enclosure: `[1/2, 3/4]`, with validated endpoint order

Operations that may grow storage return this result type:

```text
Result<_, BigintError>
```

## Usage

This example reduces a rational and then computes a negative integer power:

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

A dyadic rational stores an integer and a nonnegative binary exponent. Conversion from a finite floating-point value is lossless. Conversion back to a floating-point value uses ties-to-even rounding.

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

The public types have the following roles:

- `BigUint`: normalized arbitrary-precision natural number stored as little-endian `u32` limbs
- `BigInt`: sign and magnitude pair with one representation for zero
- `Sign`: integer sign
  - `Negative`: value below zero
  - `Zero`: zero
  - `Positive`: value above zero
- `ExtendedGcd`: greatest common divisor and two Bézout coefficients
- `RawRational`: unreduced numerator and denominator
- `RationalReduction`: input, greatest common divisor, and reduced result
- `ReducedRational`: rational with a positive denominator and coprime parts
- `Dyadic`: normalized integer divided by a power of two
- `DyadicEnclosure`: inclusive interval with validated endpoint order
- `BigintError`: failure returned by checked operations

Primitive integers use the following conversion paths:

- `BigUint`: unsigned primitive integers
- `BigInt`: signed and unsigned primitive integers
- `TryFrom`: checked construction

Copying an arbitrary-precision value can allocate memory. Every owning type therefore provides `try_clone`, which reports allocation failure.

Rational reduction has two public entries:

- `RawRational::reduce`: performs validation and reduction
- `RationalReduction`: preserves the input and greatest common divisor with the reduced value

## Failures

`BigintError` distinguishes ten failure conditions:

- `CapacityOverflow`: a limb count, bit count, shift, or iteration count exceeds the checked capacity
- `AllocationFailure { requested_limbs }`: allocation fails and the payload records the required limb count
- `UnsignedUnderflow`: natural-number subtraction would produce a negative value
- `DivisionByZero`: an arithmetic divisor is zero
- `NonExactDivision`: exact division has a nonzero remainder
- `ZeroDenominator`: rational reduction receives a zero denominator
- `NonFiniteFloat`: exact dyadic conversion receives infinity or NaN
- `FloatOutOfRange`: a dyadic magnitude exceeds the largest finite `f64` value
- `InvalidInterval`: dyadic enclosure endpoints are reversed
- `ExponentOverflow`: a dyadic exponent exceeds the supported maximum
  - `required`: exponent required by the operation
  - `maximum`: largest supported `u32` exponent

Operations that grow storage calculate the required limb count with checked arithmetic, enforce `BigUint::MAX_LIMBS`, and reserve memory through a fallible allocation path. Capacity and allocation failures remain ordinary `Result` values in both supported configurations.

## Runtime configuration

The default configuration uses the standard library:

```text
std
```

Disabling default features selects `no_std` with `alloc`:

```text
core + alloc
```

```toml
[dependencies]
neco-bigint = { version = "0.1", default-features = false }
```

When `std` is enabled, the public error type implements `std::error::Error`. Numeric behavior and failure variants are identical in both configurations.

## License

MIT License.
