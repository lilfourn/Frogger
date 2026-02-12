# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the React + TypeScript frontend. Key areas are `components/`, `hooks/`, `stores/`, `services/`, `types/`, and `styles/`. Frontend tests are colocated with source files as `*.test.ts` and `*.test.tsx`.

`src-tauri/` contains the Rust backend for Tauri. Important modules live in `src-tauri/src/commands/` (IPC handlers), `services/` (business logic), `data/` (SQLite/repository/migrations), and `models/`.

Use `tests/e2e/` for Playwright end-to-end tests. Keep static assets in `public/` or `src/assets/`.

## Build, Test, and Development Commands
Use `pnpm` (CI-standard) and Rust stable.

- `pnpm install --frozen-lockfile`: install JS dependencies.
- `pnpm tauri dev`: run the desktop app locally (Vite + Tauri).
- `pnpm build`: TypeScript check and production frontend build.
- `pnpm typecheck`: run `tsc --noEmit`.
- `pnpm lint` / `pnpm lint:fix`: lint and auto-fix frontend code.
- `pnpm test`: run Vitest unit/component tests.
- `pnpm test:e2e`: run Playwright specs from `tests/e2e/`.
- `cd src-tauri && cargo test`: run Rust tests.
- `cd src-tauri && cargo clippy -- -D warnings`: match backend CI quality gate.

## Coding Style & Naming Conventions
TypeScript runs in strict mode; avoid `any` unless justified. Formatting is enforced by Prettier (`tabWidth: 2`, semicolons, double quotes, trailing commas, 100-char print width). ESLint uses `typescript-eslint`, `react-hooks`, and `react-refresh`.

Naming patterns:
- React components: `PascalCase.tsx` (example: `FileView.tsx`)
- Hooks: `useX.ts` (example: `useFileNavigation.ts`)
- Stores/services: `*Store.ts`, `*Service.ts`
- Rust files/modules: `snake_case`

## Testing Guidelines
Place frontend tests next to implementation files and name them `*.test.ts(x)`. Prefer behavior-focused tests and add regression coverage for bug fixes. Run relevant frontend and backend tests before pushing.

No explicit coverage threshold is configured; the expectation is that changed logic is covered and CI checks pass.

## Commit & Pull Request Guidelines
Use Conventional Commits with scope when possible (examples from history: `feat(search): ...`, `fix(indexing): ...`, `perf(indexing): ...`, `chore: ...`).

PRs should include:
- A concise problem/solution summary
- Linked issue/task (if applicable)
- Test evidence (commands run)
- Screenshots or short recordings for UI changes

Keep PRs focused; avoid unrelated refactors.

## Security & Configuration Tips
Copy `.env.example` for local setup and never commit secrets. Treat shell/file-operation paths in `src-tauri/src/shell/` as security-sensitive and review them carefully when modifying command execution behavior.
