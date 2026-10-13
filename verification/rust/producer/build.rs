use std::{env, fs, io::Write, path::PathBuf, process::Command};

fn main() {
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let output = Command::new(&rustc)
        .arg("-vV")
        .output()
        .unwrap_or_else(|error| panic!("{rustc} -vV: {error}"));
    if !output.status.success() {
        panic!("{rustc} -vV exited with {}", output.status);
    }
    let identity = String::from_utf8(output.stdout).expect("rustc -vV is UTF-8");
    let target = env::var("TARGET").expect("cargo sets TARGET");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let manifest = manifest_dir.join("../../manifest");
    let semantics = manifest.join("algebra-semantics.json");
    let resource = manifest.join("algebra-resource.json");
    let projection = manifest.join("algebra-projection-declaration.json");
    let out = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    fs::write(out.join("rustc-identity.txt"), identity).expect("write rustc identity");
    fs::write(out.join("target-triple.txt"), target).expect("write target triple");
    fs::copy(&semantics, out.join("algebra-semantics.json")).expect("copy semantics declaration");
    fs::copy(&resource, out.join("algebra-resource.json")).expect("copy resource declaration");
    fs::copy(&projection, out.join("algebra-projection-declaration.json"))
        .expect("copy projection declaration");
    let instructions = format!(
        "cargo:rerun-if-changed={}\ncargo:rerun-if-changed={}\ncargo:rerun-if-changed={}\ncargo:rerun-if-env-changed=RUSTC\n",
        semantics.display(),
        resource.display(),
        projection.display()
    );
    std::io::stdout()
        .write_all(instructions.as_bytes())
        .expect("emit cargo build instructions");
}
