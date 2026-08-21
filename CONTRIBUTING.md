# Contributing to lazydb

Thanks for taking the time to contribute.

## Workflow

lazydb uses a basic GitHub flow:

1. Branch off `main` (`git checkout -b my-change`).
2. Make your change, with tests.
3. Open a pull request against `main`.
4. CI must pass; then squash-merge.

`main` is protected — all changes land through a pull request.

## Before you push

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

CI runs exactly these three, plus a build on Linux and macOS. `cargo clippy` is
gated on `-D warnings`, so a warning fails the build.

## Tests

Tests are mandatory for new behavior, and this project is written test-first:
write a failing test that captures the expected behavior, then make it pass.

Tests live inline as `#[cfg(test)] mod tests` at the bottom of each source file.
A `MockDatabase` in `src/db/mod.rs` (behind `#[cfg(test)]`) is available for
testing app logic that depends on query results.

Exempt from unit tests: UI rendering (`src/ui/*.rs`), `main.rs` glue, database
backend implementations (they need real connections), and filesystem-dependent
config loading — test those via `toml::from_str()` instead.

When changing existing behavior, add a regression test for it first.

## Local databases

`docker-compose.yaml` brings up PostgreSQL and ClickHouse instances for manual
testing:

```bash
docker compose up -d
```

## Architecture

See [CLAUDE.md](./CLAUDE.md) for a tour of the module layout and the main design
patterns (the vim state machine, flat-index tree addressing, connection profiles).
