use neco_algebra_protocol_model::{
    decode_canonical_request, AdapterFailure, GeneratedRequestBatch, OperandProvenance,
    ADAPTER_REVISION, ALGEBRA_DOMAIN, DRAW_SCHEDULE_REVISION, GENERATOR_ALGORITHM,
};

use crate::descriptor;

pub(crate) fn validate_request_batch(
    bytes: &[u8],
) -> Result<GeneratedRequestBatch, AdapterFailure> {
    let batch = decode_canonical_request(bytes)?;
    if batch.domain != ALGEBRA_DOMAIN {
        return Err(AdapterFailure::invalid_request(
            "validateRequest",
            "request domain is unsupported",
        ));
    }
    if batch.adapter_revision != ADAPTER_REVISION {
        return Err(AdapterFailure::invalid_request(
            "validateRequest",
            "request adapter revision is unsupported",
        ));
    }
    if batch.algorithm != GENERATOR_ALGORITHM {
        return Err(AdapterFailure::invalid_request(
            "validateRequest",
            "request generator algorithm is unsupported",
        ));
    }
    if batch.draw_schedule_revision != DRAW_SCHEDULE_REVISION {
        return Err(AdapterFailure::invalid_request(
            "validateRequest",
            "request draw schedule revision is unsupported",
        ));
    }
    let max_batch_cases = descriptor::max_batch_cases()?;
    let case_count = u64::try_from(batch.cases.len()).unwrap_or(u64::MAX);
    if case_count > max_batch_cases {
        return Err(batch_resource_limit(case_count, max_batch_cases));
    }
    Ok(batch)
}

fn batch_resource_limit(case_count: u64, max_batch_cases: u64) -> AdapterFailure {
    AdapterFailure::new(
        "batchResourceLimit",
        "validateRequest",
        OperandProvenance {
            case_id: None,
            family: None,
            requested_observation_kind: None,
            selectors: Default::default(),
            projection_revision: None,
            resource_revision: None,
            build_identity: None,
        },
        format!("request has {case_count} cases; resource claim permits at most {max_batch_cases}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_algebra_protocol_model::{
        canonical_request_bytes, GeneratedRequestCase, SELECTOR_NAMES,
    };
    use std::collections::BTreeMap;
    #[test]
    fn unsupported_metadata_is_an_invalid_request() {
        let selectors: BTreeMap<_, _> = SELECTOR_NAMES
            .into_iter()
            .map(|name| (name.to_owned(), 0))
            .collect();
        let batch = GeneratedRequestBatch {
            adapter_revision: "other".to_owned(),
            domain: ALGEBRA_DOMAIN.to_owned(),
            algorithm: GENERATOR_ALGORITHM.to_owned(),
            draw_schedule_revision: DRAW_SCHEDULE_REVISION.to_owned(),
            seed: "0000000000000000".to_owned(),
            cases: vec![GeneratedRequestCase {
                case_id: "case-0000000000000000".to_owned(),
                global_case_index: 0,
                domain_case_index: 0,
                family: "rational-reduction".to_owned(),
                requested_observation_kind: "success".to_owned(),
                selectors,
            }],
        };
        assert_eq!(
            validate_request_batch(&canonical_request_bytes(&batch))
                .unwrap_err()
                .kind(),
            "invalidRequest"
        );
    }
}
