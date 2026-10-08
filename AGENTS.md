# AGENTS.md — solana-pos

Working guide for agents (and humans) on this repo. Rust workspace, axum backend,
Postgres (sqlx), Dioxus/WASM frontend, Playwright e2e.

## Before starting any task

1. Read `.plans/` — highest-numbered plans are the latest context. If a plan file
   covers your task, follow it and update its checkboxes as you finish each item.
2. Check git flow: `git --no-pager status` and `git --no-pager log -n 5` to see
   the current branch and recent work. Don't commit unless asked.
3. Never read or open `.env` files.

## Project layout

```
crates/
  api/          pos-api: shared request/response schema (serde), route constants
  backend/      pos-backend: the app
    domain/         entities + value objects (validation lives here)
    app/            use-case services (invoice, destination, shop, pdf, qr)
    port/           repo/renderer traits
    infra/          http_adapter (axum router) + database_adapter (sqlx repos)
  ui/
    ui_interface/   ApiClient trait (frontend boundary)
    ui_wasm/        Dioxus UI (FetchClient implements ApiClient)
docker/         docker-compose + migrations (numbered 001..NNN)
e2e/            Playwright tests
.plans/         numbered plan files (00..NN)
bench/          benchmark notes and results
```

## Workflow for a change

1. Create/update a numbered plan in `.plans/` (next number after the highest).
2. Make the change through the layers: domain → app service → port/repo →
   http_adapter/router → api schema → UI. Keep each layer's change consistent
   (a new entity field must be threaded through every layer and the DB).
3. DB changes: add a new numbered migration in `docker/migrations/`
   (`007_xxx.sql` style). Never edit existing migrations.
4. Update tests in `crates/backend/tests/` when constructors/signatures change.

## Style rules

- Solid, small functions. Zero-copy first (`&str` over `String`, borrow first).
- Lean code, no over-engineering.
- No hard-coded values — use named `const`.
- Prefer enums over hard-coded strings (e.g. `Chain`, `SplToken`).
- Use `RwLock` instead of `Mutex` where locking is needed.
- No comments except where logic is genuinely complex.
- No `#[allow(dead_code)]` — use `todo!()` / `unimplemented!()` instead.
- No `pub use` re-exports — `pub mod` only; use `crate::` or `super::` paths.

## Finish checklist (every task)

```
cargo check --workspace
cargo clippy --workspace --all-targets
cargo test -p pos-backend        # or --workspace if UI/domain touched
```

All three must be clean before reporting done.

If the task involved something benchmarkable, guess/measure first and write a
summary in `bench/`.

Mark plan-file tasks done and add a new `.plans/NN-<topic>-plan.md` for the
next follow-up work.

## Running

```
make stack-up     # postgres + deps (docker)
make run          # backend
make ui-build     # trunk build of the WASM UI
make e2e          # playwright suite (needs stack test compose)
make test         # all cargo tests
```

## API/DB conventions

- API structs in `crates/api/src/schema.rs` mirror domain entities via explicit
  conversion fns in `router.rs` (`chain_to_api` / `token_to_domain` style).
  New optional fields need `#[serde(default)]` for backward compatibility.
- New API enum variants need: domain enum + `as_str`/`parse` + api enum +
  conversion fns + UI select mapping.
