# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Repository foundation: MIT license, README, architecture / development /
  roadmap / release / relationship docs, contribution & security guidelines,
  issue / PR templates.
- `desktop-manifest.json` — component version lock (`schemaVersion` 1) for the
  releasegraph-driven auto-upgrade flow.
- `desktop-config.example.json` — documented example config (`local` / `remote`
  modes).

### Changed

- F0 scoped to **repository skeleton only**: removed the pre-spec Rust / Tauri
  scaffolding and bundling scripts so the Foundation stage stays a pure,
  public-ready structure with **no business logic** (per the F0 definition).

## [0.1.0] - 2026-09-26

Initial foundation release. Establishes the desktop shell skeleton and the
release-ecosystem contract; no installable artifacts were published for this
version.

[Unreleased]: https://github.com/redtidev1918/pixivflow-desktop/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/redtidev1918/pixivflow-desktop/releases/tag/v0.1.0