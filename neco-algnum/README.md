# neco-algnum

[日本語](README-ja.md)

`neco-algnum` is a crate for exact real algebraic numbers.
A value is identified by a pair: an integer minimal polynomial and the index of a real root.
The minimal polynomial is kept in normal form (primitive, irreducible, positive leading coefficient), and root indices start at 0.
For example, $ \sqrt{2} + \sqrt{3} $ is represented by the pair below.

$$ m(x) = x^{4} - 10x^{2} + 1, \qquad k = 3 $$

The real roots of $ m $ are the four values $ \pm\sqrt{3} \pm \sqrt{2} $, and counting them in ascending order, the index $ k = 3 $ points to the largest root $ \sqrt{2} + \sqrt{3} $.
Certified intervals with dyadic endpoints are used to construct and observe a value.
The identity of the value itself is preserved by the minimal polynomial and root index.

## Polynomials and validated types

Integer and rational polynomials store their coefficients from the lowest degree upward.
Values under construction and validated values are separated by type.

- `Polynomial`: an integer polynomial with trailing zeroes removed
- `RationalPolynomial`: a rational polynomial with trailing zeroes removed
- `CandidatePolynomial`: a candidate polynomial of degree at least 1
- `SquareFreePolynomial`: a primitive, square-free polynomial with a positive leading coefficient
- `IrreduciblePolynomial`: a polynomial irreducible over the rationals
- `MinimalPolynomial`: the irreducible polynomial that identifies a real algebraic number
- `PolynomialQuotient`: operations in the quotient ring `Q[x] / (m)`
- `GeneratorRepresentative`: the generator `x mod m` for the same modulus
- `RootIndex`: the index of a real root of one irreducible polynomial, counted in ascending order
- `RealAlgebraic`: an exact value made of a minimal polynomial and a real-root index
- `IsolatingInterval`: a dyadic interval that contains an exact value and is used for observation
- `CertifiedAlgebraic`: a pair of an exact value and a certified interval obtained by root isolation

`Polynomial` supports addition, subtraction, multiplication, differentiation, evaluation at integer and rational points, and composition.
`CandidatePolynomial::square_free` removes the content (the greatest common divisor of the coefficients) and repeated factors.
Factorization enumerates every factor candidate with the Kronecker method. A polynomial that no enumerated candidate divides is determined to be irreducible.

### Polynomial quotient

`PolynomialQuotient` reduces operation results modulo the minimal polynomial and returns a `RationalPolynomial` of degree below the modulus.

- `reduce`: reduce one polynomial
- `add`: reduce the result of an addition
- `sub`: reduce the result of a subtraction
- `mul`: reduce the result of a multiplication

`generator` returns the generator reduced by the same modulus.
`RationalPolynomial::to_real_algebraic_coefficients` converts rational coefficients to real algebraic numbers and returns the result as `RationalCoefficientConversion`.

## Certified real roots

Sturm sequences isolate every real root into intervals that each contain exactly one distinct root, and the real roots of the same irreducible polynomial receive root indices in ascending order of value.
Neighboring intervals may share an endpoint.
A root is constructed either by isolating all real roots or by validating an interval supplied by the caller, and the exact value is then extracted from the result.

- `isolate_real_roots`: return all real roots with certified intervals
- `certify_root`: validate two caller-provided dyadic endpoints
- `into_value`: extract the exact value from a certified construction result

### Interval refinement

`RealAlgebraic::enclose` and `IsolatingInterval::refine` refine an interval to at most the requested width while preserving the same minimal polynomial and root index.

### Exact observations

Comparison, sign, and the zero and one tests return exact results.
The minimal polynomial and root index used for identification can also be read.

- `compare`: compare two real algebraic numbers
- `compare_dyadic`: compare with a dyadic value
- `sign`: determine the sign
- `is_zero`: decide whether the value is zero from the minimal polynomial and root index
- `is_one`: decide whether the value is the multiplicative unit 1 from the minimal polynomial and root index
- `minimal_polynomial`: read the minimal polynomial used for identification
- `root_index`: read the root index used for identification

## Exact algebraic operations

`RealAlgebraic` provides the four arithmetic operations, powers, and roots.
It also converts from formal sums and tests equality with them.

- `add`: add two real algebraic numbers
- `sub`: subtract two real algebraic numbers
- `mul`: multiply two real algebraic numbers
- `div`: divide two real algebraic numbers
- `pow_integer`: raise to an integer power
- `pow_rational`: raise to a reduced rational power with a real value
- `nth_root`: take a real root of positive degree
- `from_form_sum`: promote an exact `FormSum` to a real algebraic number
- `equals_form_sum`: decide equality with a formal sum. The values are equal when substituting the formal sum into the minimal polynomial with `FormSum` arithmetic gives zero, and an interval containing the value of the formal sum meets only the interval of the root that the root index points to

Addition, subtraction, multiplication, division, powers, and roots all proceed through resultant construction, removal of square factors, complete factorization, and certified root selection, and return the result again as a pair of minimal polynomial and root index (`RealAlgebraic`).
Division first tests whether the divisor is zero and rejects it if so.
An even root of a negative value has no real value and returns a failure, while an odd root of a negative value returns the unique real root.

## Failures

`AlgnumError` distinguishes input, operation, representation, storage, and lower-crate failures.

- `ZeroPolynomial`: no candidate of degree at least 1 exists
- `InvalidIsolation`: the endpoints are reversed or equal, or an endpoint is itself a root of the polynomial
- `NoTargetRoot`: the given interval contains no target root
- `MultipleTargetRoots`: the given interval contains more than one target root
- `DivisionByZero`: division by a value that is exactly zero
- `UndefinedZeroPower`: `0^0` was requested
- `ZeroToNegativePower`: zero was raised to a negative power
- `ZeroRootDegree`: a root of degree 0 was requested
- `EvenRootOfNegative`: an even root of a negative value was requested
- `RepresentationLimit`: a degree, coefficient count, or Sylvester matrix size exceeded the representation limit
- `AllocationLimit`: the exact estimate of the required total element count exceeded the `usize` limit
- `AllocationFailure`: the allocator refused an allocation; holds the resource and the requested element count
- `Bigint`: holds a `neco-bigint` failure with its variant and payload
- `FormSum`: holds a `neco-formsum` failure with its variant and payload

### Representation limits

`RepresentationResource` distinguishes the subjects that exceeded a limit:

- root degree
- polynomial degree
- coefficient count
- Sylvester matrix dimension
- Sylvester matrix element count

### Allocation subjects

`AllocationResource` distinguishes thirteen kinds of subjects whose allocation failed:

- polynomial coefficients
- rational coefficients
- Kronecker evaluation points
- divisors of the evaluated values
- indices that enumerate tuples of divisors
- factor candidates
- factors
- Sturm sequences
- root intervals
- Sylvester matrix elements
- permutations for the determinant expansion
- resultant coefficients
- root candidates

Storage failures come in two kinds: the total element count exceeds the platform limit, or the allocator refuses a request within that limit.

Public types that own variable-length values provide `try_clone` and return allocation failures during cloning through `Result`.

## Features and dependencies

With the default `std` feature enabled, `AlgnumError` can be used as a standard error type, and failures from lower crates remain reachable through the error source.
This feature also enables the feature of the same name in both dependencies.
Disabling default features selects the `core + alloc` configuration, which handles the same exact values and the same failures.

```console
cargo check -p neco-algnum --no-default-features
```

At runtime, the crate depends on `neco-bigint` and `neco-formsum`.

## License

MIT License.
