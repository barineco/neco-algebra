# neco-expr

[日本語](README-ja.md)

`neco-expr` stores exact values in expression graphs and resolves them, per consumer, into certified floating-point values. A resolved result carries a finite `f64`, a dyadic enclosure at the requested precision, and an exact absolute error bound. The descent to floating point happens only at that final step, and every intermediate value stays exact.

## Usage

This example stores the exact value one as an atom and resolves it at 20 bits of absolute precision:

```rust
use neco_expr::{
    AbsoluteBits, Assignments, AtomId, AtomStore, ConsumerId, ExactValue, ExprGraph,
    ExprNode, PrecisionRequirements, Resolver,
};
use neco_monomial::Monomial;

let mut graph = ExprGraph::new();
let expression = graph
    .push(ExprNode::Atom(AtomId::new(0)))
    .expect("valid expression node");

let mut atoms = AtomStore::new();
let atom_value = ExactValue::Monomial(Monomial::one());
atoms
    .insert(AtomId::new(0), atom_value)
    .expect("unique atom ID");

let consumer = ConsumerId::new(0);
let mut requirements = PrecisionRequirements::new();
requirements
    .insert(consumer, AbsoluteBits::new(20))
    .expect("unique consumer ID");
let mut assignments = Assignments::new();
assignments
    .insert(consumer, expression)
    .expect("unique consumer ID");

let (_, _, resolved) = Resolver::new()
    .resolve_all(&graph, &atoms, &requirements, &assignments)
    .expect("sufficient storage");
let certified = resolved
    .get(consumer)
    .expect("requested consumer")
    .as_ref()
    .expect("successful resolution");
assert_eq!(certified.value(), 1.0);
```

## Expressions and exact values

`ExactValue` has three ordered layers:

- `Monomial`: a normalized monomial
- `FormSum`: a normalized formal sum
- `Algebraic`: a real algebraic number identified by a minimal polynomial and root index

The graph is built from two types:

- `ExprGraph`: an insertion-ordered arena
- `ExprNode`: atoms, negation, arithmetic operations, and reduced rational powers

Operation nodes may reference only previously inserted nodes, so every graph is acyclic.

For example, the golden ratio is built from a square-root atom and exact constants.
The addition produces a layer-2 formal sum.
Resolution returns an enclosure within the requested width and a finite floating-point value selected from it:

```text
phi = (1 + 5^(1/2)) / 2
```

The operation and input layers determine the result layer:

| Operation | Inputs | Result |
|---|---|---|
| Negation | Any layer | Input layer |
| Addition, subtraction | Layers 1 and 2 | `FormSum` |
| Addition, subtraction | Includes layer 3 | `Algebraic` |
| Multiplication, division | Two `Monomial` values | `Monomial` |
| Multiplication, division | Up to layer 2 | `FormSum` |
| Multiplication, division | Includes layer 3 | `Algebraic` |
| Integer power | Any layer | Input layer |
| Proper rational power | `Monomial` | `Monomial` |
| Proper rational power | `FormSum` | `Algebraic` |
| Proper rational power | `Algebraic` | `Algebraic` |

## Declarations and resolution

Resolution takes four independent inputs:

- `ExprGraph`: expression nodes
- `AtomStore`: atom IDs and exact values
- `PrecisionRequirements`: consumer IDs and absolute precision
- `Assignments`: consumer IDs and expression IDs

Resolution is a single call:

- `Resolver::resolve_all`: resolves every consumer in ID order

The call returns three values:

- `EvaluationCache`: exact values or evaluation failures for reached expression IDs
- `IsolationCache`: algebraic isolating intervals reusable for the same expression ID and precision
- `ResolvedValues`: a `CertifiedF64` or resolution failure for every requested consumer

Unknown expression or atom IDs are not stored in the evaluation cache; they become resolution failures of the requesting consumer. When one consumer fails, its failure is stored in the result map and resolution continues with the remaining consumers. The outer result reports failure only when a cache or the result map itself cannot be stored.

## Certified floating-point values

`CertifiedF64` contains three values:

- `value()`: a finite `f64` selected with ties-to-even rounding
- `enclosure()`: a dyadic interval containing the exact value
- `absolute_error()`: the greater distance from the selected value to either enclosure endpoint

Absolute precision uses this input:

- `AbsoluteBits(bits)`: limits the enclosure width

```text
upper - lower <= 2^(-bits)
```

Every `u32` value, including zero, is a valid precision. Floating-point selection is independent of the requested precision. Selection normalizes negative zero to positive zero and compares exact values across all finite `f64` values, including subnormals, positive zero, and the maximum finite value.

Standalone projection uses the following public values:

- `project_exact_value_f64`: projects one exact value
- `ExactValue`: supplies the exact input
- `ProjectionPolicy`: supplies the enclosure precision
- `CertifiedScalarProjection`: retains the policy, selected value, enclosure, and absolute error

The result represents a value beyond the finite `f64` range with this failure:

- `ScalarProjectionError::FloatOutOfRange`: the exact value is beyond the finite range

## Public failures

Failures are separated by operation stage:

- `GraphError`: exhausted IDs, references to nodes yet to be inserted, node cloning, and graph storage
- `InsertError`: duplicate IDs, atom-value cloning, and input-map storage
- `EvalError`: division by zero, zero powers, even roots, and lower-crate failures
- `ResolveError`: missing declarations, unknown IDs, finite range, evaluation, enclosure, and result storage
- `ScalarProjectionError`: range, lower-crate, and storage failures of standalone projection
- `StorageError`: capacity overflow and allocation refusal with the required total element count

Lower failures and checked copying use these APIs:

- `std::error::Error::source`: references the stored lower failure
- `try_clone`: checks allocation while copying owned sequences

`try_clone` is the single copying path for types that own sequences.

## Configuration and dependencies

Runtime dependencies are the four lower crates:

- `neco-bigint`
- `neco-monomial`
- `neco-formsum`
- `neco-algnum`

The default `std` feature enables standard error integration and the same feature in the dependencies. Disabling default features selects the `core + alloc` configuration:

```bash
cargo check -p neco-expr --no-default-features
```

## License

MIT License.

## `ExactComputationProduct`

The high-level API preserves the lower-level graph API and assigns exact and numerical responsibilities to the modal-field-projection and Wavesim consumers.

- `ExactExpressionRequirement`: one consumer, its expressions, one typed exact decision, and one `AbsoluteBits` requirement
- `ExactNumericAllocation`: exact inputs, exact decisions, numerical operations, descent consumers, and owner-specific numerical error budgets

The six operations form one ownership-moving pipeline:

1. `read_exact_numeric_inputs`
2. `allocate_exact_numeric`
3. `normalize_exact_expressions`
4. `decide_exact_properties`
5. `resolve_certified_f64`
6. `assemble_exact_computation_product`

`ExactComputationProduct` returns one certified bundle for each consumer. Within one owner's expression graph, equal expression-and-precision requests share one exact-to-`f64` resolution.

`direct_inspection` exposes allocations, requirements, typed decisions, certified bundles, and the shared resolution count. It performs no solver, file, process, network, or CLI work.

All high-level failures use the closed `NecoFailure` family. It preserves the operation, consumer, expression, atom, or decision location and maps storage and lower arithmetic failures. Numerical error budgets remain owned by Modal Field Projection or Wavesim and are not added to `CertifiedF64::absolute_error()`.
