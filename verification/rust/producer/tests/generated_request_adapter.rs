use neco_algebra_protocol_model::{
    canonical_request_bytes, decode_canonical_request, validate_canonical,
};
use neco_algebra_producer::{generate_request, self_check_request};

const REQUEST: &[u8] =
    include_bytes!("../../../vectors/requests/algebra-generated-request-batch.json");
const OBSERVATION: &[u8] =
    include_bytes!("../../../vectors/requests/algebra-generated-observation-batch.json");

#[test]
fn generated_vector_has_exact_case_correspondence() {
    let request = decode_canonical_request(REQUEST).expect("canonical request");
    let generated = generate_request(REQUEST).expect("generated request succeeds");
    let observations = validate_canonical(&generated).expect("canonical observations");
    assert_eq!(request.cases.len(), observations.len());
    for (request, observation) in request.cases.iter().zip(observations.iter()) {
        assert_eq!(observation["caseId"], request.case_id);
    }
    assert_eq!(generated, OBSERVATION);
}

#[test]
fn self_check_replays_public_api_bytes() {
    self_check_request(REQUEST, OBSERVATION).expect("checked-in observation is reproducible");
    let mut supplied = OBSERVATION.to_vec();
    let position = supplied
        .windows(b"rational-reduction".len())
        .position(|window| window == b"rational-reduction")
        .expect("operation token");
    supplied[position] = b'R';
    assert_eq!(
        self_check_request(REQUEST, &supplied)
            .expect_err("changed observation is rejected")
            .kind(),
        "invalidRequest"
    );
}

#[test]
fn batch_above_resource_claim_is_a_batch_resource_limit_failure() {
    let request = decode_canonical_request(REQUEST).expect("canonical request");
    let template = request
        .cases
        .iter()
        .find(|case| case.family == "rational-reduction")
        .expect("reduction case")
        .clone();
    let expand = |count: u64| {
        let mut batch = request.clone();
        batch.cases = (0..count)
            .map(|index| {
                let mut case = template.clone();
                case.global_case_index = index;
                case.domain_case_index = index;
                case.case_id = format!("case-{index:016x}");
                case
            })
            .collect();
        canonical_request_bytes(&batch)
    };
    assert!(generate_request(&expand(2000)).is_ok());
    let failure = generate_request(&expand(2001)).expect_err("2001 cases exceed the claim");
    assert_eq!(failure.kind(), "batchResourceLimit");
}

#[test]
fn unsupported_local_root_kind_fails_before_observation_construction() {
    let mut request = decode_canonical_request(REQUEST).expect("canonical request");
    let root_case_id = {
        let root = request
            .cases
            .iter_mut()
            .find(|case| case.family == "local-algebraic-root")
            .expect("local root case");
        root.requested_observation_kind = "no-local-root".to_owned();
        root.case_id.clone()
    };
    let failure = generate_request(&canonical_request_bytes(&request))
        .expect_err("generated local root does not expose no-local-root");
    assert_eq!(failure.kind(), "unsupportedProjection");
    assert_eq!(
        failure.operand_provenance().case_id.as_deref(),
        Some(root_case_id.as_str())
    );
    assert_eq!(
        failure
            .operand_provenance()
            .requested_observation_kind
            .as_deref(),
        Some("no-local-root")
    );
}
