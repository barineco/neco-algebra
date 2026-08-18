# Contributing

## Workspace checks

Run formatting once for the whole workspace.

```sh
cargo fmt \
  --all \
  --check
```

Run checks, Clippy, and tests with both feature configurations.

```sh
cargo check \
  --workspace \
  --locked
cargo check \
  --workspace \
  --no-default-features \
  --locked
cargo clippy \
  --workspace \
  --all-targets \
  --all-features \
  --locked \
  -- -D warnings
cargo clippy \
  --workspace \
  --all-targets \
  --no-default-features \
  --locked \
  -- -D warnings
cargo test \
  --workspace \
  --all-features \
  --locked
cargo test \
  --workspace \
  --no-default-features \
  --locked
```

## Release verification

Use the direct commands below as the release gate.
The commands are maintained in this repository.
Use each command as its own wrapper.
Run the workspace checks with both feature configurations, then inspect each package in dependency order.

```sh
cargo fmt \
  --all \
  --check
cargo check \
  --workspace \
  --locked
cargo check \
  --workspace \
  --no-default-features \
  --locked
cargo test \
  --workspace \
  --all-features \
  --locked
cargo test \
  --workspace \
  --no-default-features \
  --locked
cargo clippy \
  --workspace \
  --all-targets \
  --all-features \
  --locked \
  -- -D warnings
cargo clippy \
  --workspace \
  --all-targets \
  --no-default-features \
  --locked \
  -- -D warnings
cargo package \
  -p neco-bigint \
  --list \
  --locked
cargo package \
  -p neco-complex \
  --list \
  --locked
cargo package \
  -p neco-linear-types \
  --list \
  --locked
cargo package \
  -p neco-linear-dense \
  --list \
  --locked
cargo package \
  -p neco-linear-exact \
  --list \
  --locked
cargo package \
  -p neco-generalized-eigen \
  --list \
  --locked
cargo package \
  -p neco-eigensolve \
  --list \
  --locked
cargo package \
  -p neco-sparse \
  --list \
  --locked
cargo package \
  -p neco-monomial \
  --list \
  --locked
cargo package \
  -p neco-formsum \
  --list \
  --locked
cargo package \
  -p neco-algnum \
  --list \
  --locked
cargo package \
  -p neco-expr \
  --list \
  --locked
```

## Publishing

Publish one crate at a time from a clean Git checkout in dependency order.

For each crate, confirm that the checkout is clean and inspect the package contents before publishing.

```sh
git status \
  --porcelain=v1 \
  --untracked-files=all
cargo package \
  -p <crate> \
  --list \
  --locked
cargo publish \
  -p <crate> \
  --locked
```
