use neco_algebra_producer::{generate_request, self_check_request};

const REQUEST: &[u8] =
    include_bytes!("../../../vectors/requests/algebra-generated-request-batch.json");

#[test]
fn generation_is_a_pure_function_of_the_canonical_request_and_public_api() {
    let first = generate_request(REQUEST).expect("first generation");
    let second = generate_request(REQUEST).expect("second generation");
    assert_eq!(first, second);
    self_check_request(REQUEST, &first).expect("self-check uses the same public API flow");
}

#[test]
fn checker_shaped_bytes_are_not_an_observation_authority() {
    let checker_shaped = br#"{"cases":[],"schema":"neco.verification.checker-response.v1"}"#;
    let failure = self_check_request(REQUEST, checker_shaped)
        .expect_err("checker-shaped bytes cannot replace producer observations");
    assert_eq!(failure.kind(), "invalidRequest");
}
