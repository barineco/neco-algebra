# neco-bigint

[日本語](README-ja.md)

`neco-bigint` provides arbitrary-precision arithmetic for exact computation. It covers natural numbers, integers, and reduced rationals, together with dyadic rationals and their enclosures. Operations that can grow storage report overflow and allocation failure to the caller as `Result` values.

## Features

Operations come in four groups of numbers:

- Natural numbers: addition, subtraction (failing when the result would be negative), multiplication, shifts, division, powers, gcd, and lcm
- Integers: signed arithmetic, Euclidean division, powers, and extended gcd
- Rationals: reduction with evidence, arithmetic, integer powers, floor and ceiling, and dyadic rounding
- Dyadic rationals: arithmetic on normal forms, exact conversion from and to finite `f64`, ties-to-even rounding, and enclosures

Every owning numeric type hides its representation, so only validated values can be constructed. Natural numbers, integers, reduced rationals, and dyadic rationals are always held in normal form. The single exception is `RawRational`, which exists to preserve an input before reduction.

The representations, written out as values:

- The rational `-6/8` reduces to `-3/4`, held with a positive denominator and coprime parts
- A dyadic rational has the form `m * 2^e` and represents values such as `0.625 = 5 * 2^(-3)` exactly
- The enclosure `[1/2, 3/4]` has dyadic endpoints and serves to observe an exact value

Operations that may grow storage return their result as:

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

A dyadic rational stores an integer and a nonnegative binary exponent.
Finite floating-point values convert without loss, and the reverse conversion rounds ties to even:

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

- `BigUint`: arbitrary-precision natural numbers stored as normalized little-endian `u32` limbs
- `BigInt`: a sign paired with a `BigUint` magnitude, representing zero in exactly one way
- `Sign`: the sign of an integer
  - `Negative`: a value below zero
  - `Zero`: zero
  - `Positive`: a value above zero
- `ExtendedGcd`: gcd and the two Bézout coefficients
- `RawRational`: an unreduced numerator and denominator
- `RationalReduction`: the input, gcd, and reduced result of rational reduction
- `ReducedRational`: a rational with a positive denominator and coprime parts
- `Dyadic`: a normalized integer divided by a power of two
- `DyadicEnclosure`: an inclusive interval with validated endpoint order
- `BigintError`: failures returned by checked operations

Primitive integers convert through the following implementations:

- `BigUint`: constructed from unsigned primitive integers
- `BigInt`: constructed from signed and unsigned primitive integers
- `TryFrom`: checked construction

Copying an arbitrary-precision value allocates, so every owning type provides `try_clone`, which reports the allocation outcome.

Rational reduction uses two public entries:

- `RawRational::reduce`: performs the reduction
- `RationalReduction`: preserves the input and gcd alongside the reduced value

## Failures

`BigintError` distinguishes ten failure conditions:

- `CapacityOverflow`: a limb count, bit count, shift, or iteration count exceeds its checked capacity
- `AllocationFailure { requested_limbs }`: storage allocation fails; the payload is the total required limb count
- `UnsignedUnderflow`: natural-number subtraction would produce a negative value
- `DivisionByZero`: an arithmetic divisor is zero
- `NonExactDivision`: exact division has a nonzero remainder
- `ZeroDenominator`: rational reduction receives a zero denominator
- `NonFiniteFloat`: exact dyadic conversion receives infinity or NaN
- `FloatOutOfRange`: a dyadic magnitude exceeds the maximum finite `f64` value
- `InvalidInterval`: dyadic enclosure endpoints are reversed
- `ExponentOverflow`: a dyadic exponent exceeds the supported maximum
  - `required`: the exponent required by the operation
  - `maximum`: the largest supported `u32` exponent

Operations that grow storage first calculate the total limb count with checked arithmetic, enforce `BigUint::MAX_LIMBS`, and then reserve memory through a fallible allocation path. Capacity and allocation failures therefore remain ordinary `Result` values in both supported configurations.

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

In the `std` configuration the public error type implements `std::error::Error`. Numeric behavior and failure variants are the same in both configurations.

## License

MIT License.
