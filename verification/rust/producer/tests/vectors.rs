use neco_algebra_protocol_model::{decode_canonical_request, validate_canonical};
use neco_algebra_producer::{generate_request, self_check_request};

const REQUEST: &[u8] =
    include_bytes!("../../../vectors/requests/algebra-generated-request-batch.json");
const OBSERVATION: &[u8] =
    include_bytes!("../../../vectors/requests/algebra-generated-observation-batch.json");

#[test]
fn checked_in_generated_vectors_are_canonical_and_reproducible() {
    let request = decode_canonical_request(REQUEST).expect("request vector is canonical");
    let observation = validate_canonical(OBSERVATION).expect("observation vector is canonical");
    assert_eq!(request.cases.len(), observation.len());
    assert_eq!(
        generate_request(REQUEST).expect("generate vector"),
        OBSERVATION
    );
    self_check_request(REQUEST, OBSERVATION).expect("vector parity");
}

#[test]
fn request_vector_uses_each_family_once_and_local_root_success_only() {
    let request = decode_canonical_request(REQUEST).expect("request vector is canonical");
    let families: Vec<_> = request
        .cases
        .iter()
        .map(|case| case.family.as_str())
        .collect();
    assert_eq!(
        families,
        [
            "rational-reduction",
            "extended-gcd-bezout",
            "local-algebraic-root",
            "scalar-f64-projection",
            "rational-linear-unique",
            "rational-linear-affine",
        ]
    );
    let root = request
        .cases
        .iter()
        .find(|case| case.family == "local-algebraic-root")
        .expect("local root case");
    assert_eq!(root.requested_observation_kind, "success");
}
