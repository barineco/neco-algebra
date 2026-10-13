use neco_algebra_protocol_model::{AdapterFailure, OperandProvenance};
use serde_json::Value;

use crate::projection_declaration::ProjectedRequest;
use crate::{gcd, projection, reduction};

pub(crate) fn invoke(invocation: &ProjectedRequest) -> Result<Value, AdapterFailure> {
    let arguments = &invocation.invocation.arguments;
    let result = match invocation.family {
        "rational-reduction" => reduction(
            number(arguments, "numerator")?,
            unsigned(arguments, "denominator")?,
            &invocation.case_id,
        ),
        "extended-gcd-bezout" => gcd(
            number(arguments, "left")?,
            number(arguments, "right")?,
            &invocation.case_id,
        ),
        "local-algebraic-root" => crate::observe_root(
            arguments,
            &invocation.case_id,
            &invocation.requested_observation_kind,
        )?,
        "scalar-f64-projection" => projection(
            dyadic_integer(arguments)?,
            dyadic_exponent(arguments)?,
            unsigned(arguments, "shift")? as u32,
            unsigned(arguments, "bits")? as u32,
            &invocation.case_id,
        )
        .map_err(|error| {
            failure(
                invocation,
                "publicApiConstructionFailure",
                error.to_string(),
            )
        })?,
        "rational-linear-unique" => crate::observe_unique(
            arguments,
            &invocation.case_id,
            &invocation.requested_observation_kind,
        )?,
        "rational-linear-affine" => crate::observe_affine(
            arguments,
            &invocation.case_id,
            &invocation.requested_observation_kind,
        )?,
        _ => {
            return Err(failure(
                invocation,
                "unsupportedProjection",
                "public operation is absent",
            ))
        }
    };
    Ok(result)
}

fn number(value: &Value, field: &str) -> Result<i64, AdapterFailure> {
    value
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| invalid(field))
}
fn unsigned(value: &Value, field: &str) -> Result<u64, AdapterFailure> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .or_else(|| {
            value
                .get(field)
                .and_then(Value::as_str)
                .and_then(|value| value.parse().ok())
        })
        .ok_or_else(|| invalid(field))
}
fn dyadic_integer(value: &Value) -> Result<i64, AdapterFailure> {
    value
        .get("value")
        .and_then(|value| value.get("integer"))
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| invalid("value.integer"))
}
fn dyadic_exponent(value: &Value) -> Result<u32, AdapterFailure> {
    value
        .get("value")
        .and_then(|value| value.get("exponent"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| invalid("value.exponent"))
}
fn invalid(field: &str) -> AdapterFailure {
    AdapterFailure::invalid_request(
        "invokeProductPublicApi",
        format!("projected argument {field} is invalid"),
    )
}
fn failure(
    request: &ProjectedRequest,
    kind: &'static str,
    condition: impl Into<String>,
) -> AdapterFailure {
    AdapterFailure::new(
        kind,
        "invokeProductPublicApi",
        OperandProvenance {
            case_id: Some(request.case_id.clone()),
            family: Some(request.family.to_owned()),
            requested_observation_kind: Some(request.requested_observation_kind.clone()),
            selectors: Default::default(),
            projection_revision: None,
            resource_revision: None,
            build_identity: None,
        },
        condition,
    )
}
