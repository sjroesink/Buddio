# Repository Guidelines

## Project Structure & Module Organization
Buddio is a Rust + Tauri + React monorepo.
- `src/`: React UI (`components/`, `hooks/`, `types.ts`, `App.tsx`).
- `src-tauri/`: Desktop app shell (`src/commands.rs`, `tauri.conf.json`, icons, capabilities).
- `golaunch-core/`: Shared Rust library for SQLite access and data models.
- `golaunch-cli/`: Rust CLI crate for item CRUD/import/export (builds `buddio-cli`).
- `.github/workflows/`: CI/release pipelines. Build outputs go to `target/` and frontend artifacts to `dist/`.

## Build, Test, and Development Commands
Run from repository root unless noted.
- `npm ci`: install frontend dependencies exactly as locked.
- `npm run dev`: run Vite frontend only.
- `npm run build`: TypeScript compile + frontend production bundle.
- `npx tauri dev`: run full desktop app in development mode.
- `npx tauri build`: produce desktop release artifacts.
- `cargo build --workspace`: compile all Rust crates.
- `cargo build --release --package buddio-cli`: build release CLI binary.
- `cargo fmt --all -- --check`: Rust formatting check.
- `cargo clippy --workspace -- -D warnings`: lint Rust (warnings fail CI).
- `npx tsc --noEmit`: strict TS type check.

## Coding Style & Naming Conventions
- TypeScript/React: 2-space indentation, functional components, `PascalCase` for component files (e.g., `ItemList.tsx`), `camelCase` for hooks/functions, `use*` hook naming.
- Rust: `rustfmt` defaults, `snake_case` modules/functions, `CamelCase` types.
- Keep modules focused by feature boundary (`core` data logic, `cli` command interface, `src-tauri` desktop commands).

## Testing Guidelines
There is no dedicated automated test suite yet. Minimum validation before PR:
- `npm run build`
- `npx tsc --noEmit`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo build --workspace`
Add new tests close to changed code (`#[cfg(test)]` in Rust modules, component/hook tests if frontend test tooling is introduced).

## Commit & Pull Request Guidelines
Use Conventional Commits (example in history: `feat: initial Buddio ...`). Prefer:
- `feat: ...`, `fix: ...`, `chore: ...`, `docs: ...`, `refactor: ...`.

PRs should include:
- Clear summary and rationale.
- Linked issue (if applicable).
- Validation commands run and results.
- UI screenshots/GIFs for `src/` or Tauri window changes.
- Notes on DB/schema or CLI behavior changes.

## Agent Interaction
- If the agent identifies an action to perform, it should propose that action to the user before executing it.
- When the agent believes it has completed the user's request, it should suggest a relevant next action/command the user can run (if applicable).

