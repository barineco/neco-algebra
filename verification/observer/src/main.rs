use neco_algebra_observer::{
    canonical_batch, canonical_batch_bytes, check_expected_digest, inspect_files, run_lean_check,
    sha256, verdict_counts, verification_root, InspectionFailure,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

enum Arguments {
    Verify(Mode),
    Inspection {
        batch: PathBuf,
        checker_result: PathBuf,
    },
}

enum Mode {
    Vectors(PathBuf),
    Smoke(usize),
}

#[derive(Clone, Copy)]
enum VectorExpectation {
    Known,
    Tampered,
    Invalid,
}

fn parse_arguments(mut arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let flag = arguments.next().ok_or_else(usage)?;
    let value = arguments.next().ok_or_else(usage)?;
    if flag == "--batch" {
        let result_flag = arguments.next().ok_or_else(usage)?;
        let result = arguments.next().ok_or_else(usage)?;
        if result_flag != "--checker-result" || arguments.next().is_some() {
            return Err(usage());
        }
        return Ok(Arguments::Inspection {
            batch: PathBuf::from(value),
            checker_result: PathBuf::from(result),
        });
    }
    if arguments.next().is_some() {
        return Err(usage());
    }
    let mode = match flag.as_str() {
        "--vectors" => Mode::Vectors(PathBuf::from(value)),
        "--smoke" => Mode::Smoke(
            value
                .parse()
                .map_err(|_| "--smoke は正の整数でなければなりません".to_owned())?,
        ),
        _ => return Err(usage()),
    };
    Ok(Arguments::Verify(mode))
}

fn usage() -> String {
    "usage: neco-algebra-observer --vectors <verification/vectors/{known,tampered,invalid}> | --smoke <case-count> | --batch <canonical-observation-batch> --checker-result <canonical-checker-result>"
        .to_owned()
}

fn canonical_path(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn vector_batches(vectors: &Path, root: &Path) -> Result<Vec<PathBuf>, String> {
    let allowed_root = canonical_path(&root.join("vectors"))?;
    let vectors = canonical_path(vectors)?;
    if !vectors.starts_with(&allowed_root) {
        return Err(format!(
            "{} は observer manifest の vectors 許可範囲外です",
            vectors.display()
        ));
    }
    let mut pending = vec![vectors];
    let mut batches = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(&path).map_err(|error| format!("{}: {error}", path.display()))? {
            let entry = entry.map_err(|error| error.to_string())?;
            let entry_path = entry.path();
            if entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                pending.push(entry_path);
            } else if entry_path
                .extension()
                .is_some_and(|extension| extension == "json" || extension == "hex")
            {
                batches.push(entry_path);
            }
        }
    }
    batches.sort();
    if batches.is_empty() {
        return Err("vector directory に JSON batch がありません".to_owned());
    }
    Ok(batches)
}

fn decode_hex_vector(path: &Path) -> Result<Vec<u8>, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let hex = text.strip_suffix('\n').unwrap_or(&text);
    if hex.is_empty()
        || hex.len() % 2 != 0
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!(
            "{}: lowercase hex bytes ではありません",
            path.display()
        ));
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16).map_err(|error| error.to_string())
        })
        .collect()
}

fn vector_expectation(vectors: &Path, root: &Path) -> Result<VectorExpectation, String> {
    let allowed_root = canonical_path(&root.join("vectors"))?;
    let vectors = canonical_path(vectors)?;
    let relative = vectors.strip_prefix(allowed_root).map_err(|_| {
        format!(
            "{} は observer manifest の vectors 許可範囲外です",
            vectors.display()
        )
    })?;
    match relative
        .components()
        .next()
        .and_then(|component| component.as_os_str().to_str())
    {
        Some("known") => Ok(VectorExpectation::Known),
        Some("tampered") => Ok(VectorExpectation::Tampered),
        Some("invalid") => Ok(VectorExpectation::Invalid),
        _ => Err("vectors は known, tampered, invalid のいずれかを指定してください".to_owned()),
    }
}

fn satisfies_expectation(expectation: VectorExpectation, counts: &BTreeMap<String, usize>) -> bool {
    match expectation {
        VectorExpectation::Known => all_verified(counts),
        VectorExpectation::Tampered => {
            counts.values().sum::<usize>() > 0
                && counts
                    .keys()
                    .all(|verdict| verdict == "refuted" || verdict == "invalid")
        }
        VectorExpectation::Invalid => {
            counts.values().sum::<usize>() > 0
                && counts.len() == 1
                && counts.contains_key("invalid")
        }
    }
}

fn all_verified(counts: &BTreeMap<String, usize>) -> bool {
    counts.values().sum::<usize>() > 0 && counts.len() == 1 && counts.contains_key("verified")
}

fn staged_lean_executable(root: &Path) -> Result<PathBuf, String> {
    let executable = root.join("artifacts/lean/neco-algebra-checker");
    if !executable.is_file() {
        return Err(format!(
            "{} がありません。Lean と Rust の個別 build/test 後に staged artifact を配置してください",
            executable.display()
        ));
    }
    Ok(executable)
}

fn staged_producer_batch(root: &Path) -> Result<PathBuf, String> {
    let batch = root.join("artifacts/rust/algebra-observation-batch.json");
    if !batch.is_file() {
        return Err(format!(
            "{} がありません。Lean と Rust の個別 build/test 後に staged artifact を配置してください",
            batch.display()
        ));
    }
    Ok(batch)
}

fn write_digest(root: &Path, digest: &str, cases: &serde_json::Value) -> Result<(), String> {
    let directory = root.join("artifacts/digests");
    fs::create_dir_all(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let output = json!({"cases": cases, "digest": digest});
    let bytes = neco_algebra_observer::canonical_json(&output)?;
    fs::write(directory.join(format!("{digest}.json")), bytes)
        .map_err(|error| format!("{}: {error}", directory.display()))
}

fn evaluate_batch(
    root: &Path,
    executable: &Path,
    batch: &Path,
) -> Result<(String, BTreeMap<String, usize>), String> {
    let bytes = canonical_batch(batch)?;
    let digest = sha256(&bytes);
    check_expected_digest(batch, &digest)?;
    let result = run_lean_check(executable, batch, &digest)?;
    let counts = verdict_counts(&result.cases)?;
    write_digest(root, &digest, &result.cases)?;
    Ok((digest, counts))
}

enum CliFailure {
    PlainMessage(String),
    Inspection(InspectionFailure),
}

fn emit_inspection_failure(failure: InspectionFailure) {
    let bytes = failure.into_report().to_string();
    let _ = std::io::stderr().write_all(bytes.as_bytes());
    let _ = std::io::stderr().write_all(b"\n");
}

fn main() {
    let mut arguments = env::args().skip(1).peekable();
    let inspection_requested = arguments.peek().is_some_and(|flag| flag == "--batch");
    let result = match parse_arguments(arguments) {
        Ok(Arguments::Verify(mode)) => run(mode).map_err(CliFailure::PlainMessage),
        Ok(Arguments::Inspection {
            batch,
            checker_result,
        }) => inspect_files(&batch, &checker_result)
            .and_then(|value| {
                let bytes = neco_algebra_observer::canonical_json(&value).map_err(|error| {
                    InspectionFailure::process_failure("encodeInspectionReport", error)
                })?;
                String::from_utf8(bytes).map_err(|error| {
                    InspectionFailure::process_failure("encodeInspectionReport", error.to_string())
                })
            })
            .map_err(CliFailure::Inspection),
        Err(error) if inspection_requested => Err(CliFailure::Inspection(
            InspectionFailure::invalid_invocation(error),
        )),
        Err(error) => Err(CliFailure::PlainMessage(error)),
    };
    match result {
        Ok(output) => {
            if inspection_requested {
                let mut stdout = std::io::stdout().lock();
                if let Err(error) = stdout
                    .write_all(output.as_bytes())
                    .and_then(|_| stdout.write_all(b"\n"))
                {
                    emit_inspection_failure(InspectionFailure::process_failure(
                        "writeInspectionReport",
                        error.to_string(),
                    ));
                    std::process::exit(2);
                }
            } else {
                let _ = std::io::stdout().write_all(output.as_bytes());
                let _ = std::io::stdout().write_all(b"\n");
            }
        }
        Err(CliFailure::Inspection(failure)) => {
            emit_inspection_failure(failure);
            std::process::exit(2);
        }
        Err(CliFailure::PlainMessage(error)) => {
            let _ =
                std::io::stderr().write_all(format!("neco-algebra-observer: {error}\n").as_bytes());
            std::process::exit(1);
        }
    }
}

fn run(mode: Mode) -> Result<String, String> {
    let root = verification_root();
    let executable = staged_lean_executable(&root)?;
    let producer_batch = staged_producer_batch(&root)?;
    match mode {
        Mode::Vectors(vectors) => {
            let expectation = vector_expectation(&vectors, &root)?;
            let mut total: BTreeMap<String, usize> = BTreeMap::new();
            for batch in vector_batches(&vectors, &root)? {
                let counts = if batch
                    .extension()
                    .is_some_and(|extension| extension == "hex")
                {
                    match decode_hex_vector(&batch)
                        .and_then(|bytes| canonical_batch_bytes(&batch, bytes))
                    {
                        Ok(_) => {
                            return Err(format!(
                                "{} は invalid bytes を表さなければなりません",
                                batch.display()
                            ))
                        }
                        Err(_) => BTreeMap::from([("invalid".to_owned(), 1)]),
                    }
                } else {
                    match evaluate_batch(&root, &executable, &batch) {
                        Ok((_, counts)) => counts,
                        Err(_) => BTreeMap::from([("invalid".to_owned(), 1)]),
                    }
                };
                if !satisfies_expectation(expectation, &counts) {
                    return Err(format!(
                        "{} は指定した vector の期待結果を満たしません",
                        batch.display()
                    ));
                }
                for (verdict, count) in counts {
                    *total.entry(verdict).or_default() += count;
                }
            }
            if matches!(expectation, VectorExpectation::Known) && total.get("verified") != Some(&13)
            {
                return Err(
                    "known vector は 13 件すべてが verified でなければなりません".to_owned(),
                );
            }
            let output = json!({"cases": total, "digest": "vectors"});
            Ok(String::from_utf8(neco_algebra_observer::canonical_json(&output)?).unwrap())
        }
        Mode::Smoke(required_cases) => {
            if required_cases == 0 {
                return Err("--smoke は正の整数でなければなりません".to_owned());
            }
            let bytes = canonical_batch(&producer_batch)?;
            let batch = serde_json::from_slice::<serde_json::Value>(&bytes)
                .map_err(|error| error.to_string())?;
            let observed_cases = batch["cases"]
                .as_array()
                .ok_or_else(|| "producer batch の cases は array でなければなりません".to_owned())?
                .len();
            if observed_cases < required_cases {
                return Err(format!(
                    "producer batch は {observed_cases} 件です。smoke には {required_cases} 件以上が必要です"
                ));
            }
            let (digest, counts) = evaluate_batch(&root, &executable, &producer_batch)?;
            if !all_verified(&counts) {
                return Err("smoke の全 case は verified でなければなりません".to_owned());
            }
            let output = json!({"cases": counts, "digest": digest});
            Ok(String::from_utf8(neco_algebra_observer::canonical_json(&output)?).unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_verified_requires_nonempty_verified_cases_only() {
        assert!(all_verified(&BTreeMap::from([(
            "verified".to_owned(),
            1000
        )])));
        assert!(!all_verified(&BTreeMap::new()));
        assert!(!all_verified(&BTreeMap::from([
            ("verified".to_owned(), 999),
            ("refuted".to_owned(), 1),
        ])));
        assert!(!all_verified(&BTreeMap::from([("invalid".to_owned(), 1)])));
    }
}
