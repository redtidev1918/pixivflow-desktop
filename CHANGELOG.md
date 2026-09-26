# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0](https://github.com/redtidev1918/pixivflow-desktop/compare/v0.1.0...v0.2.0) (2026-09-26)


### Features

* acquire real PixivFlow runtime into the bundle (F2.3) ([9fdb76f](https://github.com/redtidev1918/pixivflow-desktop/commit/9fdb76ffcae1f63971ac9f3b9edb8ca6a8ef7483))
* **auth:** run the Pixiv login in an in-app window (F4.1) ([2c40b84](https://github.com/redtidev1918/pixivflow-desktop/commit/2c40b84026ca3cef8eac6b4a9050ead8a3f3ac10))
* **backend:** forward the system proxy to the runtime (F4.1) ([a248e66](https://github.com/redtidev1918/pixivflow-desktop/commit/a248e66322c2194683b8b4c08f4d03981d346f13))
* bundle the real runtime and WebUI, auto-open it (F4.1) ([0ec583b](https://github.com/redtidev1918/pixivflow-desktop/commit/0ec583bc964130a75f28e396897c0d7cbf05be38))
* close macOS packaging loop with runtime resource resolution (F4.0) ([0e4b104](https://github.com/redtidev1918/pixivflow-desktop/commit/0e4b10443a34f231d0debe0f431fea1edd068815))
* **desktop:** WebUI-first window model and embedded Pixiv sign-in ([cb75bcc](https://github.com/redtidev1918/pixivflow-desktop/commit/cb75bccc24a3e2f4f90f21f7a14044d65a61c978))
* **diagnostics:** app log system, Rust i18n, diagnostics export (F4.1) ([bb3fb84](https://github.com/redtidev1918/pixivflow-desktop/commit/bb3fb84bd6af7717f30d9fc1ae4493b503b1809c))
* **f1.1:** desktop bootstrap UI, white-screen fix, product naming ([8dee3cb](https://github.com/redtidev1918/pixivflow-desktop/commit/8dee3cbfa6c8abb971308b7e208d81d1ad0d708b))
* **f1:** Desktop Shell MVP — Tauri window, BackendManager lifecycle, config, health ([774651f](https://github.com/redtidev1918/pixivflow-desktop/commit/774651f9b2e9ff3390ec897160f3e662bb1e523a))
* **f21:** real backend adapter — discovery, command adapter, backend_doctor, tests ([5ff8f35](https://github.com/redtidev1918/pixivflow-desktop/commit/5ff8f35d7ced184f1588926acaf09fa225979853))
* **f22:** bundled runtime + manifest, 方案 A WebUI integration ([821e595](https://github.com/redtidev1918/pixivflow-desktop/commit/821e5950a210489b5ba3d6de75b378f9444e3f85))
* formalize real PixivFlow runtime contract (F2.3) ([9cff3a5](https://github.com/redtidev1918/pixivflow-desktop/commit/9cff3a59538584b2018317aa682d59a57b6751f5))
* **host:** expose notify, openExternal and openUrl to the WebUI bridge ([8074cc6](https://github.com/redtidev1918/pixivflow-desktop/commit/8074cc6c0cfc86d4b049ec1d6dcebffc4935e589))
* **host:** expose reveal_directory to the WebUI bridge ([081e97d](https://github.com/redtidev1918/pixivflow-desktop/commit/081e97d98cc813b743b2e14189e96abb03e333f3))
* **host:** reveal a path instead of opening a directory ([13e22fc](https://github.com/redtidev1918/pixivflow-desktop/commit/13e22fc203d680efda9016147d830bcd05d432c1))
* **release:** publish installers through the fleet release engine ([fef0443](https://github.com/redtidev1918/pixivflow-desktop/commit/fef044356dfdff5d49e70e049c40722e84c4e6f4))
* **ui:** follow the system language in the launcher ([4b29454](https://github.com/redtidev1918/pixivflow-desktop/commit/4b294547f0fd9b813c488134d562a80bcdb8b24d))


### Bug Fixes

* **host:** a download directory set in the user config is revealable too ([09334d1](https://github.com/redtidev1918/pixivflow-desktop/commit/09334d1fda31e05a7831a13d5a7048137e618958))
* **host:** confine reveal_path to the download directories before opening ([083ebb8](https://github.com/redtidev1918/pixivflow-desktop/commit/083ebb88b70ceedb3d35447a5c63df72638deec2))
* **release:** publish only after every bundle is built ([cd327ec](https://github.com/redtidev1918/pixivflow-desktop/commit/cd327ec35f76cab0d94f0692017e0ea7cf20cbff))
* **runtime:** discover the sibling PixivFlow checkout from the repo root ([836143a](https://github.com/redtidev1918/pixivflow-desktop/commit/836143adf8bb9227e58a39d890cdeeccdef4168b))
* **runtime:** materialize the published package with npm, not tar ([cd6e6a7](https://github.com/redtidev1918/pixivflow-desktop/commit/cd6e6a7ff6e8f561e756dc5dc178ed4eb998a67a))
* **scripts:** resolve the npm shim through the shell on Windows ([b4848e2](https://github.com/redtidev1918/pixivflow-desktop/commit/b4848e2d2009965a564d7a58ce657d4ae5b1feb7))

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
