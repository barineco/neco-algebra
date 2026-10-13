use neco_algebra_protocol_model::validate_canonical;
use neco_algebra_producer::{fixture_batch, generate_request};

const REQUEST: &[u8] =
    include_bytes!("../../../vectors/requests/algebra-generated-request-batch.json");

#[test]
fn generated_successes_match_the_six_public_api_families() {
    let generated = generate_request(REQUEST).expect("generated request succeeds");
    let generated = validate_canonical(&generated).expect("canonical observation batch");
    let direct = fixture_batch();
    assert_eq!(generated.len(), direct.len());
    for (generated, direct) in generated.iter().zip(direct.iter()) {
        assert_eq!(generated["operation"], direct["operation"]);
        assert_eq!(generated["observation"]["kind"], "success");
        assert_eq!(direct["observation"]["kind"], "success");
    }
}

#[test]
fn generated_local_root_domain_contains_success_only() {
    let generated = generate_request(REQUEST).expect("generated request succeeds");
    let generated = validate_canonical(&generated).expect("canonical observation batch");
    let roots: Vec<_> = generated
        .iter()
        .filter(|case| case["operation"] == "local-algebraic-root")
        .collect();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0]["observation"]["kind"], "success");
    assert!(roots[0]["observation"].get("failure").is_none());
}
