#![forbid(unsafe_code)]

mod canonical_observation;
mod case_binding;
mod descriptor;
mod generated_request;
mod observation_authority;
mod process_render;
mod product_api_port;
mod projection_declaration;

use neco_algnum::Polynomial;
use neco_bigint::{BigInt, BigUint, BigintError, Dyadic, RawRational, ReducedRational, Sign};
use neco_expr::{
    project_exact_value_f64, AbsoluteBits, ExactValue, ProjectionPolicy, ScalarProjectionError,
};
use neco_formsum::{RawFormSum, RawTerm};
use neco_linear_exact::{ExactLinearError, ExactLinearSolution, ExactMatrix};
use neco_linear_types::{LinearError, Shape, Vector};
use neco_monomial::RawMonomial;
use neco_algebra_protocol_model::{
    canonical_descriptor_bytes, decimal, dyadic, failure, object, observation as case, rational,
    success, AdapterFailure, OperandProvenance,
};
use serde_json::Value;

const SEED: u64 = 0x9e37_79b9_7f4a_7c15;
pub const INITIAL_RANGE: i64 = 8;
pub const PRODUCER_REVISION: &str = "1";

pub fn describe() -> Result<Vec<u8>, AdapterFailure> {
    descriptor::describe().map(|descriptor| canonical_descriptor_bytes(&descriptor))
}

pub fn generate_request(
    bytes: &[u8],
) -> Result<Vec<u8>, neco_algebra_protocol_model::AdapterFailure> {
    let cases = case_binding::generate(bytes)?;
    Ok(canonical_observation::encode(&cases))
}

pub fn self_check_request(
    request: &[u8],
    supplied: &[u8],
) -> Result<(), neco_algebra_protocol_model::AdapterFailure> {
    let cases = case_binding::generate(request)?;
    canonical_observation::verify_supplied(supplied, &cases)
}

pub fn render_adapter_failure(failure: neco_algebra_protocol_model::AdapterFailure) -> Vec<u8> {
    process_render::render_failure(failure)
}

pub fn producer_metadata() -> Value {
    object([
        ("initialRange", decimal(INITIAL_RANGE)),
        (
            "producerRevision",
            Value::String(PRODUCER_REVISION.to_owned()),
        ),
        ("seed", Value::String(format!("{SEED:016x}"))),
    ])
}

fn int(value: i64) -> BigInt {
    BigInt::try_from(value).expect("initial range fits BigInt")
}
fn natural(value: u64) -> BigUint {
    BigUint::try_from(value).expect("initial range fits BigUint")
}
fn big_decimal(value: &BigUint) -> String {
    if value.is_zero() {
        return "0".to_owned();
    }
    let base = BigUint::try_from(1_000_000_000_u32).expect("base");
    let mut power = BigUint::one().expect("one");
    while power.mul(&base).is_ok_and(|next| next <= *value) {
        power = power.mul(&base).expect("bounded");
    }
    let (leading, mut rest) = value.div_rem(&power).expect("division");
    let mut text = leading.to_u32().expect("chunk").to_string();
    while power > BigUint::one().expect("one") {
        power = power.exact_div(&base).expect("exact");
        let (chunk, next) = rest.div_rem(&power).expect("division");
        text.push_str(&format!("{:09}", chunk.to_u32().expect("chunk")));
        rest = next;
    }
    text
}
fn int_decimal(value: &BigInt) -> String {
    let magnitude = big_decimal(value.magnitude());
    match value.sign() {
        Sign::Negative => format!("-{magnitude}"),
        Sign::Zero | Sign::Positive => magnitude,
    }
}
fn rational_value(value: &ReducedRational) -> Value {
    rational(
        int_decimal(value.numerator()),
        big_decimal(value.denominator()),
    )
}
fn dyadic_value(value: &Dyadic) -> Value {
    dyadic(int_decimal(value.integer()), value.exponent())
}
fn input_rational(numerator: i64, denominator: u64) -> Value {
    object([
        ("denominator", decimal(denominator)),
        ("numerator", decimal(numerator)),
    ])
}

pub fn fixture_batch() -> Vec<Value> {
    vec![
        reduction(8, 6, "reduction-001"),
        gcd(30, 18, "gcd-001"),
        root("root-001"),
        projection(3, 1, 0, 2, "projection-001").expect("fixture projection"),
        unique("unique-001"),
        affine("affine-001"),
    ]
}

pub fn smoke_batch(count: usize) -> Vec<Value> {
    let mut state = SEED;
    let mut batch = Vec::with_capacity(count.saturating_mul(6));
    let root_certificate = root("smoke-root");
    let projection_certificate =
        projection(3, 1, 0, 4, "smoke-projection").expect("smoke projection");
    for index in 0..count {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let value = (state % (INITIAL_RANGE as u64 * 2 + 1)) as i64 - INITIAL_RANGE;
        batch.push(reduction(
            value,
            (state >> 8) % INITIAL_RANGE as u64 + 1,
            &format!("smoke-{index}-reduction"),
        ));
        batch.push(gcd(value, value + 1, &format!("smoke-{index}-gcd")));
        batch.push(with_case_id(
            root_certificate.clone(),
            format!("smoke-{index}-root"),
        ));
        batch.push(with_case_id(
            projection_certificate.clone(),
            format!("smoke-{index}-projection"),
        ));
        batch.push(unique(&format!("smoke-{index}-unique")));
        batch.push(affine(&format!("smoke-{index}-affine")));
    }
    batch
}

fn with_case_id(mut certificate: Value, case_id: String) -> Value {
    certificate
        .as_object_mut()
        .expect("certificate envelope")
        .insert("caseId".to_owned(), Value::String(case_id));
    certificate
}

pub fn reduction(numerator: i64, denominator: u64, id: &str) -> Value {
    let input = input_rational(numerator, denominator);
    let observation = match RawRational::new(int(numerator), natural(denominator)).reduce() {
        Ok(value) => success(object([
            (
                "denominator",
                decimal(big_decimal(value.reduced().denominator())),
            ),
            (
                "numerator",
                decimal(int_decimal(value.reduced().numerator())),
            ),
        ])),
        Err(BigintError::ZeroDenominator) => failure("zero-denominator"),
        Err(BigintError::DivisionByZero) => failure("division-by-zero"),
        Err(error) => panic!("rational reduction producer error: {error}"),
    };
    case(id.to_owned(), input, observation, "rational-reduction")
}

pub fn gcd(left: i64, right: i64, id: &str) -> Value {
    let result = int(left)
        .extended_gcd(&int(right))
        .expect("small extended gcd");
    case(
        id.to_owned(),
        object([("left", decimal(left)), ("right", decimal(right))]),
        success(object([
            ("bezoutLeft", decimal(int_decimal(result.x()))),
            ("bezoutRight", decimal(int_decimal(result.y()))),
            ("gcd", decimal(big_decimal(result.gcd()))),
        ])),
        "extended-gcd-bezout",
    )
}

pub fn root(id: &str) -> Value {
    let polynomial = Polynomial::from_coefficients(vec![int(-2), int(0), int(1)]);
    let observation = match polynomial
        .candidate()
        .and_then(|value| value.square_free())
        .and_then(|value| value.isolate_real_roots())
    {
        Ok(mut roots) => match roots.pop() {
            Some(root) => success(object([
                ("left", dyadic_value(root.enclosure().lower())),
                ("right", dyadic_value(root.enclosure().upper())),
            ])),
            None => failure("no-local-root"),
        },
        Err(error) => panic!("local root producer error: {error}"),
    };
    case(
        id.to_owned(),
        object([(
            "polynomial",
            Value::Array(vec![decimal(-2), decimal(0), decimal(1)]),
        )]),
        observation,
        "local-algebraic-root",
    )
}

pub fn projection(
    integer: i64,
    exponent: u32,
    shift: u32,
    bits: u32,
    id: &str,
) -> Result<Value, String> {
    let scale = BigUint::one()
        .and_then(|one| one.shl_bits(shift as usize))
        .map_err(|error| error.to_string())?;
    let integer = int(integer)
        .mul(&BigInt::from_sign_magnitude(Sign::Positive, scale))
        .map_err(|error| error.to_string())?;
    projection_from_dyadic(Dyadic::new(integer, exponent), shift > 0, bits, id)
}

fn projection_from_dyadic(
    source: Dyadic,
    scaled: bool,
    bits: u32,
    id: &str,
) -> Result<Value, String> {
    let denominator = BigUint::one()
        .map_err(|error| ScalarProjectionError::Bigint(error).to_string())?
        .shl_bits(source.exponent() as usize)
        .map_err(|error| ScalarProjectionError::Bigint(error).to_string())?;
    let constant = RawRational::new(
        source
            .integer()
            .try_clone()
            .map_err(|error| ScalarProjectionError::Bigint(error).to_string())?,
        denominator,
    );
    let input = object([
        ("bits", Value::Number(bits.into())),
        ("value", dyadic_value(&source)),
    ]);
    let exact = if scaled {
        ExactValue::FormSum(
            RawFormSum::new(vec![RawTerm::new(
                constant,
                RawMonomial::positive(Vec::new()),
            )])
            .normalize()
            .map_err(|error| format!("{error:?}"))?,
        )
    } else {
        let reduced = constant
            .reduce()
            .map_err(|error| ScalarProjectionError::Bigint(error).to_string())?
            .into_reduced();
        neco_algnum::RealAlgebraic::from_reduced_rational(&reduced)
            .map(ExactValue::Algebraic)
            .map_err(|error| ScalarProjectionError::Algnum(error).to_string())?
    };
    let observation =
        match project_exact_value_f64(&exact, ProjectionPolicy::new(AbsoluteBits::new(bits))) {
            Ok(value) => success(object([
                ("absoluteError", dyadic_value(value.absolute_error())),
                (
                    "bits",
                    Value::String(format!("{:016x}", value.value().to_bits())),
                ),
                ("lower", dyadic_value(value.enclosure().lower())),
                ("upper", dyadic_value(value.enclosure().upper())),
            ])),
            Err(ScalarProjectionError::FloatOutOfRange) => failure("non-finite-f64"),
            Err(error) => return Err(error.to_string()),
        };
    Ok(case(
        id.to_owned(),
        input,
        observation,
        "scalar-f64-projection",
    ))
}

fn q(value: i64) -> ReducedRational {
    ReducedRational::from_bigint(int(value)).expect("integer rational")
}
fn unique_input() -> Value {
    object([
        (
            "matrix",
            Value::Array(vec![Value::Array(vec![rational(1, 1)])]),
        ),
        ("rhs", Value::Array(vec![rational(2, 1)])),
    ])
}
fn affine_input() -> Value {
    object([
        (
            "matrix",
            Value::Array(vec![Value::Array(vec![rational(1, 1), rational(0, 1)])]),
        ),
        ("rhs", Value::Array(vec![rational(2, 1)])),
    ])
}
fn unique_matrix() -> ExactMatrix<ReducedRational> {
    ExactMatrix::from_row_major(Shape::new(1, 1), vec![q(1)]).expect("shape")
}
fn affine_matrix() -> ExactMatrix<ReducedRational> {
    ExactMatrix::from_row_major(Shape::new(1, 2), vec![q(1), q(0)]).expect("shape")
}
fn singular_matrix() -> ExactMatrix<ReducedRational> {
    ExactMatrix::from_row_major(Shape::new(1, 1), vec![q(0)]).expect("shape")
}

pub fn unique(id: &str) -> Value {
    unique_from_parts(
        unique_matrix(),
        Vector::try_from_vec(vec![q(2)]).expect("vector"),
        unique_input(),
        id,
        "success",
    )
    .expect("square invertible fixture")
}
pub fn affine(id: &str) -> Value {
    affine_from_parts(
        affine_matrix(),
        Vector::try_from_vec(vec![q(2)]).expect("vector"),
        affine_input(),
        id,
        "success",
    )
    .expect("full row rank fixture")
}

pub(crate) fn observe_root(
    arguments: &Value,
    id: &str,
    requested: &str,
) -> Result<Value, AdapterFailure> {
    let coefficients = arguments
        .get("polynomial")
        .and_then(Value::as_array)
        .ok_or_else(|| argument_failure("polynomial"))?;
    let mut parsed = Vec::new();
    for coefficient in coefficients {
        let text = coefficient
            .as_str()
            .ok_or_else(|| argument_failure("polynomial"))?;
        let value: i64 = text
            .parse()
            .map_err(|_| argument_failure("polynomial"))?;
        parsed.push(int(value));
    }
    let polynomial = Polynomial::from_coefficients(parsed);
    let observation = match polynomial
        .candidate()
        .and_then(|value| value.square_free())
        .and_then(|value| value.isolate_real_roots())
    {
        Ok(mut roots) => match roots.pop() {
            Some(root) => success(object([
                ("left", dyadic_value(root.enclosure().lower())),
                ("right", dyadic_value(root.enclosure().upper())),
            ])),
            None => failure("no-local-root"),
        },
        Err(error) => {
            return Err(AdapterFailure::new(
                "publicApiConstructionFailure",
                "observeRoot",
                OperandProvenance {
                    case_id: Some(id.to_owned()),
                    family: Some("local-algebraic-root".to_owned()),
                    requested_observation_kind: Some(requested.to_owned()),
                    selectors: Default::default(),
                    projection_revision: None,
                    resource_revision: None,
                    build_identity: None,
                },
                error.to_string(),
            ))
        }
    };
    Ok(case(
        id.to_owned(),
        arguments.clone(),
        observation,
        "local-algebraic-root",
    ))
}

pub(crate) fn observe_unique(
    arguments: &Value,
    id: &str,
    requested: &str,
) -> Result<Value, AdapterFailure> {
    let (matrix, rhs) = linear_parts(arguments)?;
    unique_from_parts(matrix, rhs, arguments.clone(), id, requested)
}

pub(crate) fn observe_affine(
    arguments: &Value,
    id: &str,
    requested: &str,
) -> Result<Value, AdapterFailure> {
    let (matrix, rhs) = linear_parts(arguments)?;
    affine_from_parts(matrix, rhs, arguments.clone(), id, requested)
}

fn linear_parts(
    arguments: &Value,
) -> Result<(ExactMatrix<ReducedRational>, Vector<ReducedRational>), AdapterFailure> {
    let rows = arguments
        .get("matrix")
        .and_then(Value::as_array)
        .ok_or_else(|| argument_failure("matrix"))?;
    let mut values = Vec::new();
    let mut columns = None;
    for row in rows {
        let cells = row
            .as_array()
            .ok_or_else(|| argument_failure("matrix"))?;
        match columns {
            None => columns = Some(cells.len()),
            Some(expected) if expected != cells.len() => {
                return Err(argument_failure("matrix"));
            }
            Some(_) => {}
        }
        for cell in cells {
            values.push(rational_from_value(cell)?);
        }
    }
    let row_count = rows.len();
    let column_count = columns.unwrap_or(0);
    let matrix = ExactMatrix::from_row_major(Shape::new(row_count, column_count), values)
        .map_err(|error| argument_failure(&error.to_string()))?;
    let rhs_cells = arguments
        .get("rhs")
        .and_then(Value::as_array)
        .ok_or_else(|| argument_failure("rhs"))?;
    let mut rhs_values = Vec::new();
    for cell in rhs_cells {
        rhs_values.push(rational_from_value(cell)?);
    }
    let rhs = Vector::try_from_vec(rhs_values).map_err(|error| argument_failure(&error.to_string()))?;
    Ok((matrix, rhs))
}

fn rational_from_value(value: &Value) -> Result<ReducedRational, AdapterFailure> {
    let numerator: i64 = value
        .get("numerator")
        .and_then(Value::as_str)
        .and_then(|text| text.parse().ok())
        .ok_or_else(|| argument_failure("numerator"))?;
    let denominator: u64 = value
        .get("denominator")
        .and_then(Value::as_str)
        .and_then(|text| text.parse().ok())
        .ok_or_else(|| argument_failure("denominator"))?;
    RawRational::new(int(numerator), natural(denominator))
        .reduce()
        .map(|value| value.into_reduced())
        .map_err(|_| argument_failure("rational"))
}

fn argument_failure(field: &str) -> AdapterFailure {
    AdapterFailure::invalid_request("invokeProductPublicApi", format!("projected argument {field} is invalid"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LinearProjectedClass {
    SquareFullColumnRank,
    RectangularFullRowRank,
    SquareRankDeficient,
    RhsLengthMismatch,
    OtherDeclaredInputClass,
}

type LinearOutcome = Result<ExactLinearSolution<ReducedRational>, ExactLinearError>;

struct LinearCase<'a> {
    id: &'a str,
    family: &'static str,
    requested: &'a str,
}

fn linear_projected_class(
    matrix: &ExactMatrix<ReducedRational>,
    rhs: &Vector<ReducedRational>,
    linear_case: &LinearCase,
) -> Result<LinearProjectedClass, AdapterFailure> {
    if rhs.len() != matrix.rows() {
        return Ok(LinearProjectedClass::RhsLengthMismatch);
    }
    let rank = matrix.rank().map_err(|error| {
        linear_adapter_failure(
            "publicApiConstructionFailure",
            linear_case,
            error.to_string(),
        )
    })?;
    let rows = matrix.rows();
    let columns = matrix.columns();
    Ok(if rows == columns && rank == columns {
        LinearProjectedClass::SquareFullColumnRank
    } else if rows == columns {
        LinearProjectedClass::SquareRankDeficient
    } else if rows < columns && rank == rows {
        LinearProjectedClass::RectangularFullRowRank
    } else {
        LinearProjectedClass::OtherDeclaredInputClass
    })
}

fn linear_adapter_failure(
    kind: &'static str,
    linear_case: &LinearCase,
    condition: String,
) -> AdapterFailure {
    AdapterFailure::new(
        kind,
        "classifyPublicApiOutcome",
        OperandProvenance {
            case_id: Some(linear_case.id.to_owned()),
            family: Some(linear_case.family.to_owned()),
            requested_observation_kind: Some(linear_case.requested.to_owned()),
            selectors: Default::default(),
            projection_revision: None,
            resource_revision: None,
            build_identity: None,
        },
        condition,
    )
}

fn unexpected_linear_outcome(
    linear_case: &LinearCase,
    outcome: &LinearOutcome,
    class: LinearProjectedClass,
) -> AdapterFailure {
    let outcome = match outcome {
        Ok(ExactLinearSolution::Unique(_)) => "Unique".to_owned(),
        Ok(ExactLinearSolution::Affine { .. }) => "Affine".to_owned(),
        Ok(ExactLinearSolution::Inconsistent) => "Inconsistent".to_owned(),
        Err(error) => error.to_string(),
    };
    linear_adapter_failure(
        "unexpectedPublicApiOutcome",
        linear_case,
        format!("{outcome} with {class:?}"),
    )
}

fn is_rhs_length_mismatch(outcome: &LinearOutcome) -> bool {
    matches!(
        outcome,
        Err(ExactLinearError::Linear(
            LinearError::StorageLengthMismatch { .. }
        ))
    )
}

fn unique_from_parts(
    matrix: ExactMatrix<ReducedRational>,
    rhs: Vector<ReducedRational>,
    input: Value,
    id: &str,
    requested: &str,
) -> Result<Value, AdapterFailure> {
    let linear_case = LinearCase {
        id,
        family: "rational-linear-unique",
        requested,
    };
    let class = linear_projected_class(&matrix, &rhs, &linear_case)?;
    let outcome = matrix.solve(&rhs);
    let observation = match (&outcome, class) {
        (Ok(ExactLinearSolution::Unique(solution)), LinearProjectedClass::SquareFullColumnRank) => {
            success(object([(
                "solution",
                Value::Array(solution.values().iter().map(rational_value).collect()),
            )]))
        }
        (
            Ok(ExactLinearSolution::Affine { .. } | ExactLinearSolution::Inconsistent),
            LinearProjectedClass::SquareRankDeficient,
        ) => failure("singular-system"),
        (_, LinearProjectedClass::RhsLengthMismatch) if is_rhs_length_mismatch(&outcome) => {
            failure("dimension-mismatch")
        }
        _ => return Err(unexpected_linear_outcome(&linear_case, &outcome, class)),
    };
    Ok(case(id.to_owned(), input, observation, linear_case.family))
}

fn affine_from_parts(
    matrix: ExactMatrix<ReducedRational>,
    rhs: Vector<ReducedRational>,
    input: Value,
    id: &str,
    requested: &str,
) -> Result<Value, AdapterFailure> {
    let linear_case = LinearCase {
        id,
        family: "rational-linear-affine",
        requested,
    };
    let class = linear_projected_class(&matrix, &rhs, &linear_case)?;
    let outcome = matrix.solve(&rhs);
    let observation = match (&outcome, class) {
        (
            Ok(ExactLinearSolution::Affine { particular, kernel }),
            LinearProjectedClass::RectangularFullRowRank,
        ) => success(object([
            (
                "basis",
                Value::Array(
                    kernel
                        .iter()
                        .map(|vector| {
                            Value::Array(vector.values().iter().map(rational_value).collect())
                        })
                        .collect(),
                ),
            ),
            (
                "particular",
                Value::Array(particular.values().iter().map(rational_value).collect()),
            ),
        ])),
        (
            Ok(ExactLinearSolution::Affine { .. } | ExactLinearSolution::Inconsistent),
            LinearProjectedClass::SquareRankDeficient,
        ) => failure("singular-system"),
        (_, LinearProjectedClass::RhsLengthMismatch) if is_rhs_length_mismatch(&outcome) => {
            failure("dimension-mismatch")
        }
        _ => return Err(unexpected_linear_outcome(&linear_case, &outcome, class)),
    };
    Ok(case(id.to_owned(), input, observation, linear_case.family))
}

pub fn typed_failures() -> Vec<Value> {
    vec![
        reduction(1, 0, "failure-zero-denominator"),
        projection(1, 0, 1024, 2, "failure-non-finite-f64").expect("out of range fixture"),
        dimension_mismatch("failure-dimension-mismatch"),
        singular_unique("failure-singular-system"),
    ]
}

fn dimension_mismatch(id: &str) -> Value {
    let error = unique_matrix()
        .solve(&Vector::try_from_vec(Vec::new()).expect("empty vector"))
        .expect_err("empty right-hand side does not match one-row matrix");
    assert!(matches!(
        error,
        ExactLinearError::Linear(LinearError::StorageLengthMismatch {
            expected: 1,
            actual: 0
        })
    ));
    case(
        id.to_owned(),
        object([
            (
                "matrix",
                Value::Array(vec![Value::Array(vec![rational(1, 1)])]),
            ),
            ("rhs", Value::Array(Vec::new())),
        ]),
        failure("dimension-mismatch"),
        "rational-linear-unique",
    )
}

fn singular_unique(id: &str) -> Value {
    let solution = singular_matrix()
        .solve(&Vector::try_from_vec(vec![q(0)]).expect("vector"))
        .expect("solve");
    assert!(matches!(solution, ExactLinearSolution::Affine { .. }));
    case(
        id.to_owned(),
        object([
            (
                "matrix",
                Value::Array(vec![Value::Array(vec![rational(0, 1)])]),
            ),
            ("rhs", Value::Array(vec![rational(0, 1)])),
        ]),
        failure("singular-system"),
        "rational-linear-unique",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_algebra_protocol_model::canonical_bytes;
    #[test]
    fn public_api_parity_success_six_family_observations() {
        let batch = fixture_batch();
        assert_eq!(batch.len(), 6);
        assert!(batch
            .iter()
            .all(|case| case["observation"]["kind"] == "success"));
        let operations: Vec<_> = batch
            .iter()
            .map(|case| case["operation"].as_str())
            .collect();
        assert_eq!(
            operations,
            [
                Some("rational-reduction"),
                Some("extended-gcd-bezout"),
                Some("local-algebraic-root"),
                Some("scalar-f64-projection"),
                Some("rational-linear-unique"),
                Some("rational-linear-affine"),
            ]
        );
        assert_eq!(
            batch[2]["observation"]["value"]["left"],
            object([
                ("exponent", Value::Number(3.into())),
                ("integer", decimal(9))
            ])
        );
        assert_eq!(
            batch[2]["observation"]["value"]["right"],
            object([
                ("exponent", Value::Number(1.into())),
                ("integer", decimal(3))
            ])
        );
        assert_eq!(
            batch[3]["observation"]["value"]["absoluteError"],
            object([
                ("exponent", Value::Number(3.into())),
                ("integer", decimal(1))
            ])
        );
    }
    #[test]
    fn typed_failure_observations_follow_the_public_api() {
        let cases = typed_failures();
        let operations: Vec<_> = cases
            .iter()
            .map(|case| case["operation"].as_str())
            .collect();
        assert_eq!(
            operations,
            [
                Some("rational-reduction"),
                Some("scalar-f64-projection"),
                Some("rational-linear-unique"),
                Some("rational-linear-unique"),
            ]
        );
        let failures: Vec<_> = cases
            .iter()
            .map(|case| case["observation"]["failure"].as_str())
            .collect();
        assert_eq!(
            failures,
            [
                Some("zero-denominator"),
                Some("non-finite-f64"),
                Some("dimension-mismatch"),
                Some("singular-system"),
            ]
        );
    }
    #[test]
    fn metadata_exposes_the_deterministic_smoke_parameters() {
        assert_eq!(producer_metadata()["seed"], "9e3779b97f4a7c15");
        assert_eq!(producer_metadata()["initialRange"], "8");
        assert_eq!(producer_metadata()["producerRevision"], PRODUCER_REVISION);
    }
    #[test]
    fn smoke_is_deterministic() {
        assert_eq!(
            canonical_bytes(&smoke_batch(2)),
            canonical_bytes(&smoke_batch(2))
        );
    }
    fn linear_arguments(matrix: &[&[i64]], rhs: &[i64]) -> Value {
        object([
            (
                "matrix",
                Value::Array(
                    matrix
                        .iter()
                        .map(|row| {
                            Value::Array(row.iter().map(|value| rational(*value, 1)).collect())
                        })
                        .collect(),
                ),
            ),
            (
                "rhs",
                Value::Array(rhs.iter().map(|value| rational(*value, 1)).collect()),
            ),
        ])
    }
    fn observed_failure(observed: &Value) -> Option<&str> {
        observed["observation"]["failure"].as_str()
    }
    #[test]
    fn square_rank_deficient_systems_are_singular_in_both_families() {
        for rhs in [0, 1] {
            let arguments = linear_arguments(&[&[0]], &[rhs]);
            let unique =
                observe_unique(&arguments, "unique", "singular-system").expect("classified");
            let affine =
                observe_affine(&arguments, "affine", "singular-system").expect("classified");
            assert_eq!(observed_failure(&unique), Some("singular-system"));
            assert_eq!(observed_failure(&affine), Some("singular-system"));
        }
    }
    #[test]
    fn full_row_rank_rectangular_system_is_an_affine_success() {
        let arguments = linear_arguments(&[&[1, 0]], &[0]);
        let affine = observe_affine(&arguments, "affine", "success").expect("classified");
        assert_eq!(affine["observation"]["kind"], "success");
        let unique = observe_unique(&arguments, "unique", "success").expect_err("unclassified");
        assert_eq!(unique.kind(), "unexpectedPublicApiOutcome");
    }
    #[test]
    fn square_full_rank_system_is_an_unexpected_affine_outcome() {
        let arguments = linear_arguments(&[&[1]], &[2]);
        let unique = observe_unique(&arguments, "unique", "success").expect("classified");
        assert_eq!(unique["observation"]["kind"], "success");
        let affine = observe_affine(&arguments, "affine", "success").expect_err("unclassified");
        assert_eq!(affine.kind(), "unexpectedPublicApiOutcome");
        assert_eq!(
            affine
                .operand_provenance()
                .requested_observation_kind
                .as_deref(),
            Some("success")
        );
    }
    #[test]
    fn overdetermined_systems_are_unexpected_outcomes() {
        for rhs in [[1, 1], [1, 2]] {
            let arguments = linear_arguments(&[&[1], &[1]], &rhs);
            for observed in [
                observe_unique(&arguments, "unique", "success"),
                observe_affine(&arguments, "affine", "success"),
            ] {
                let failure = observed.expect_err("unclassified");
                assert_eq!(failure.kind(), "unexpectedPublicApiOutcome");
            }
        }
    }
    #[test]
    fn rhs_length_mismatch_is_a_dimension_mismatch() {
        let arguments = linear_arguments(&[&[1]], &[]);
        let unique =
            observe_unique(&arguments, "unique", "dimension-mismatch").expect("classified");
        let affine =
            observe_affine(&arguments, "affine", "dimension-mismatch").expect("classified");
        assert_eq!(observed_failure(&unique), Some("dimension-mismatch"));
        assert_eq!(observed_failure(&affine), Some("dimension-mismatch"));
    }
}
