# Contributing

Thanks for contributing to **PixivFlow Desktop** — the desktop shell of the
PixivFlow ecosystem.

This project is intentionally a **thin shell**. Please keep that in mind.

## Issue rules

- Search existing issues before opening a new one.
- Use the issue templates (`bug report` / `feature request`) when they apply.
- Include reproduction steps for bugs: OS/version, what you ran, expected vs
  actual.
- Report security issues privately via [SECURITY.md](SECURITY.md) — not in a
  public issue.

## PR rules

- Keep changes **small and focused**. One logical change per PR.
- **No large refactors.** If a change rewrites a subsystem, discuss it first.
- **Keep compatibility with PixivFlow**: the desktop must not break existing
  PixivFlow / webui / Docker / deploy workflows. Call this out explicitly in the
  PR's `## Compatibility` section.
- **No business logic** in the desktop. Business logic lives upstream in
  PixivFlow.
- The desktop must never fork, rebundle or re-implement upstream source.
- Do **not** bump `desktop-manifest.json` ad-hoc — version bumps belong to
  release / upgrade PRs.
- Update docs if behavior changes.
- Add a `CHANGELOG.md` entry under `[Unreleased]` for user-facing changes.
- Every PR must pass CI (when CI exists) before merge.

## Process

1. Open an issue or grab one.
2. Branch from `main`.
3. Make your change + docs.
4. Open a PR using the PR template.
5. Address review; merge after approval and green checks.

We value **simple, stable, compatible-first** contributions.