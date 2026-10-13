use neco_algebra_protocol_model::{AdapterFailure, OperandProvenance};
use serde_json::Value;

use crate::projection_declaration::ProjectedRequest;

pub(crate) fn classify(
    request: &ProjectedRequest,
    observed: Value,
) -> Result<Value, AdapterFailure> {
    let kind = observed
        .get("observation")
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            failure(
                request,
                "publicApiConstructionFailure",
                "public result lacks observation kind",
            )
        })?;
    match kind {
        "success" => Ok(observed),
        "failure" if observed["observation"]["failure"].is_string() => Ok(observed),
        "failure" => Err(failure(
            request,
            "publicApiConstructionFailure",
            "public result lacks failure kind",
        )),
        _ => Err(failure(
            request,
            "unexpectedPublicApiOutcome",
            "public result has an unknown observation kind",
        )),
    }
}

pub(crate) fn require_requested_kind_match(
    request: &ProjectedRequest,
    classified: Value,
) -> Result<Value, AdapterFailure> {
    let observation = &classified["observation"];
    let observed_kind = match observation["kind"].as_str() {
        Some("failure") => observation["failure"].as_str().unwrap_or_default(),
        kind => kind.unwrap_or_default(),
    };
    if observed_kind == request.requested_observation_kind {
        Ok(classified)
    } else {
        Err(failure(
            request,
            "unexpectedPublicApiOutcome",
            format!(
                "observed {observed_kind} for requested {}",
                request.requested_observation_kind
            ),
        ))
    }
}
fn failure(
    request: &ProjectedRequest,
    kind: &'static str,
    condition: impl Into<String>,
) -> AdapterFailure {
    AdapterFailure::new(
        kind,
        "classifyPublicApiOutcome",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection_declaration::project;
    use neco_algebra_protocol_model::{failure as failed, GeneratedRequestCase, SELECTOR_NAMES};

    fn projected(requested: &str) -> ProjectedRequest {
        let selectors = SELECTOR_NAMES
            .into_iter()
            .map(|name| (name.to_owned(), 0))
            .collect();
        project(&GeneratedRequestCase {
            case_id: "case-0000000000000000".to_owned(),
            global_case_index: 0,
            domain_case_index: 0,
            family: "rational-linear-unique".to_owned(),
            requested_observation_kind: requested.to_owned(),
            selectors,
        })
        .expect("projectable")
    }

    #[test]
    fn observation_of_another_kind_is_an_unexpected_outcome() {
        let observed = serde_json::json!({ "observation": failed("singular-system") });
        let request = projected("singular-system");
        let classified = classify(&request, observed).expect("classified");
        for requested in ["success", "dimension-mismatch"] {
            let error = require_requested_kind_match(&projected(requested), classified.clone())
                .expect_err("mismatch");
            assert_eq!(error.kind(), "unexpectedPublicApiOutcome");
        }
        assert!(require_requested_kind_match(&request, classified).is_ok());
    }
}
