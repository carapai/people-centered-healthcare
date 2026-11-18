# AGENTS.md

Instructions for coding agents working in this repository.

## Authoritative specification

`MASTER_PROMPT.md` is the authoritative project specification. Do not diverge from it without an accepted ADR in `docs/adr/` that records the approved deviation.

## Process

1. Architectural decisions are charted and resolved with the Wayfinder skill. The map is the GitHub issue labelled `wayfinder:map`. Decision tickets are its sub-issues, labelled `wayfinder:<type>`.
2. Resolved decisions are recorded as ADRs in `docs/adr/` and in the architecture docs under `docs/architecture/`.
3. Implementation tasks are separate GitHub issues (not `wayfinder:` labelled), grouped by milestone, with native blocking dependencies.
4. Implement one unblocked task at a time. Before starting, read `docs/planning/current-state.md`. After finishing, update it.
5. Never claim a check passed unless it was executed successfully in the session.

## Tooling

Bun is the only JavaScript runtime and package manager; Node.js is not installed or assumed anywhere.

- Every tool in a `package.json` script or a workflow runs as `bunx --bun <tool>` (most npm CLIs' shebangs ask for `node`). Root `bunfig.toml` sets `[run] bun = true` as a backstop, but Bun reads it only from the directory it starts in. Package install scripts that need `node` are skipped (`--ignore-scripts`) and run with `bun` instead.
- Type definitions come from `@types/bun` (which carries the `node:*` modules), never `@types/node` directly. Every `.ts` file is covered by a `tsconfig.json`; repository-level scripts use the root one.
- CI puts a `node` that fails on `PATH` in the client, design system and documentation jobs, so any Node dependency fails the build.
- Exceptions: none. A tool that cannot run on Bun is listed here with the reason before it is used.

## Repository root

This directory is the `health-platform/` root described in the specification. Do not create a nested `health-platform/` folder.

## Reference deployment

Uganda is the first reference deployment context (UgandaEMR/OpenMRS, DHIS2, mobile money, national identifiers and payers). Country-specific behaviour lives in configuration packages, never in generic engines.

## Agent skills

### Issue tracker

GitHub Issues on `carapai/people-centered-healthcare` via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
