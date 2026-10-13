#![forbid(unsafe_code)]

use std::{
    env,
    io::{Read, Write},
    process::ExitCode,
};

use neco_algebra_protocol_model::{AdapterFailure, OperandProvenance};
use neco_algebra_producer::{
    describe, generate_request, render_adapter_failure, self_check_request,
};

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    match dispatch(&arguments) {
        Ok(output) => emit(&output, false),
        Err(failure) => emit(&render_adapter_failure(failure), true),
    }
}

fn dispatch(arguments: &[String]) -> Result<Vec<u8>, AdapterFailure> {
    match arguments {
        [command] if command == "describe" => describe(),
        [command, flag, request] if command == "generate" && flag == "--request" => {
            generate_request(&read_input(request)?)
        }
        [command, request_flag, request, batch_flag, batch]
            if command == "self-check"
                && request_flag == "--request"
                && batch_flag == "--batch" =>
        {
            let request = read_input(request)?;
            let batch = read_input(batch)?;
            self_check_request(&request, &batch).map(|()| {
                neco_algebra_protocol_model::canonical_value_bytes(
                    &serde_json::json!({
                        "schema":"neco.verification.public-api-parity-result.v1",
                        "verified":true
                    }),
                )
            })
        }
        _ => Err(process_failure(
            "decodeProcessArguments",
            "expected describe, generate --request <path|->, or self-check --request <path|-> --batch <path|->",
        )),
    }
}

fn read_input(path: &str) -> Result<Vec<u8>, AdapterFailure> {
    if path == "-" {
        let mut bytes = Vec::new();
        std::io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| process_failure("readProcessInput", error.to_string()))?;
        Ok(bytes)
    } else {
        std::fs::read(path).map_err(|error| process_failure("readProcessInput", error.to_string()))
    }
}

fn process_failure(operation: &'static str, condition: impl Into<String>) -> AdapterFailure {
    AdapterFailure::new(
        "invalidProcessInvocation",
        operation,
        OperandProvenance {
            case_id: None,
            family: None,
            requested_observation_kind: None,
            selectors: Default::default(),
            projection_revision: None,
            resource_revision: None,
            build_identity: None,
        },
        condition,
    )
}

fn emit(output: &[u8], error: bool) -> ExitCode {
    let result = if error {
        std::io::stderr().lock().write_all(output)
    } else {
        std::io::stdout().lock().write_all(output)
    };
    match (result, error) {
        (Ok(()), false) => ExitCode::SUCCESS,
        _ => ExitCode::from(1),
    }
}
