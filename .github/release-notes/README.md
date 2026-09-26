# Release notes

One file per release: `.github/release-notes/<version>.md`, where `<version>` is
the version release-please writes into `.release-please-manifest.json`.

The notes are **Chinese** and must keep the `## 本次更新` heading — the
`release-metadata` CI job refuses a release pull request whose notes for the
manifest version are missing, untranslated, or missing that heading. The
`release.notes.language: "zh"` setting in `.release-policy.yml` is what makes
releasegraph use these notes instead of the English commit lines, and it appends
a machine-readable table of the published installers to whatever you write here.

Why a hand-written file exists at all: the commit log answers *what changed*,
the download page answers *what to install*, and only this file can answer *why
this release matters* for someone who runs the app rather than the repository.

Write it for a user who already has an older version installed: what they get,
what they must know before upgrading, and anything they should stop doing.
