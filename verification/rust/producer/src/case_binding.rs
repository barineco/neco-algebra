use neco_algebra_protocol_model::{AdapterFailure, GeneratedRequestBatch, GeneratedRequestCase};
use serde_json::Value;

use crate::projection_declaration::{project, ProjectedRequest};
use crate::{generated_request, observation_authority, product_api_port};

pub(crate) struct CaseBound<T> {
    value: T,
}

pub(crate) fn bind_validated_case_occurrences(
    batch: GeneratedRequestBatch,
) -> Vec<CaseBound<GeneratedRequestCase>> {
    batch
        .cases
        .into_iter()
        .map(|value| CaseBound { value })
        .collect()
}

pub(crate) fn project_case(
    case: CaseBound<GeneratedRequestCase>,
) -> Result<CaseBound<ProjectedRequest>, AdapterFailure> {
    Ok(CaseBound {
        value: project(&case.value)?,
    })
}
pub(crate) fn classify_public_api_outcome(
    case: CaseBound<ProjectedRequest>,
) -> Result<CaseBound<Value>, AdapterFailure> {
    let observed = product_api_port::invoke(&case.value)?;
    let classified = observation_authority::classify(&case.value, observed)?;
    Ok(CaseBound {
        value: observation_authority::require_requested_kind_match(&case.value, classified)?,
    })
}
pub(crate) fn construct_observed_case(case: CaseBound<Value>) -> Value {
    case.value
}

pub(crate) fn generate(bytes: &[u8]) -> Result<Vec<Value>, AdapterFailure> {
    let batch = generated_request::validate_request_batch(bytes)?;
    bind_validated_case_occurrences(batch)
        .into_iter()
        .map(|case| {
            let projected = project_case(case)?;
            let observed = classify_public_api_outcome(projected)?;
            Ok(construct_observed_case(observed))
        })
        .collect()
}
