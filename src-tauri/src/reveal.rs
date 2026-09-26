//! Host side of "show this path in my file manager" (`reveal_path`).
//!
//! The WebUI hands this command a path the backend already resolved and
//! confined (`GET /api/files/location`). The host does not take that on trust:
//! every request is confined *again*, here, against the directories this
//! installation actually downloads into. Without that check the command would
//! be a general-purpose "open anything on this machine" primitive for whatever
//! runs inside the WebUI window — `~/.ssh/id_rsa` included.
//!
//! The backend stays the authority on *where* a file is (it owns the config and
//! the database). This module owns only the two things the backend cannot do:
//! deciding whether handing that path to an OS opener is acceptable on *this*
//! machine, and starting that opener.
//!
//! Failures are coded, because "there is no file manager here" (a headless
//! Linux host, a container image) is not the same event as "you asked for a
//! path you may not see": the WebUI can offer the clipboard for the first and
//! must refuse the second.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The requested path is not inside any directory this installation downloads
/// to. Never opens anything.
pub const FORBIDDEN: &str = "REVEAL_FORBIDDEN";
/// Neither the path nor its parent directory exists any more.
pub const NOT_FOUND: &str = "REVEAL_NOT_FOUND";
/// No file manager could be started on this machine: the honest answer for a
/// host that has no desktop (headless Linux, a container), not a user error.
pub const UNAVAILABLE: &str = "REVEAL_UNAVAILABLE";
/// The file manager started and reported a failure of its own.
pub const FAILED: &str = "REVEAL_FAILED";

/// A coded, single-line error. The code stays machine-readable for the WebUI;
/// the sentence after it is what a human reads in a log.
fn error(code: &str, message: &str) -> String {
    format!("{code}: {message}")
}

/// The platform whose file manager this build talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Windows,
    Linux,
}

/// Which platform this build was compiled for.
///
/// `cfg!` (not `#[cfg]`) so every branch is compiled on every target: the
/// invocation matrix is then unit-tested on one machine instead of being
/// verified only on the machine that happens to run it.
pub fn current_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

/// The program and arguments that show `path` in `platform`'s file manager.
///
/// macOS and Windows can *select* a file inside its folder (`open -R path`,
/// `explorer /select,path`); Linux has no portable select verb, so a file
/// reveals its parent directory — the honest equivalent, not a silent no-op.
/// A directory is opened as itself on every platform.
pub fn invocation(platform: Platform, path: &str, is_directory: bool) -> (String, Vec<String>) {
    match platform {
        Platform::Macos => {
            let mut args = Vec::new();
            if !is_directory {
                args.push("-R".to_string());
            }
            args.push(path.to_string());
            ("open".to_string(), args)
        }
        Platform::Windows => {
            let argument = if is_directory {
                path.to_string()
            } else {
                format!("/select,{path}")
            };
            ("explorer".to_string(), vec![argument])
        }
        Platform::Linux => {
            let directory = if is_directory {
                path.to_string()
            } else {
                Path::new(path)
                    .parent()
                    .map(|parent| parent.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string())
            };
            ("xdg-open".to_string(), vec![directory])
        }
    }
}

/// Resolve `path` as far as it exists, so a comparison against a canonical root
/// cannot be fooled by a symlink or by `..`.
///
/// A path that does not exist yet (a stale history row) is resolved through its
/// parent, which is exactly what confinement needs: the directory is real, only
/// the file name is gone.
fn canonical_candidate(path: &Path) -> Option<PathBuf> {
    if let Ok(real) = std::fs::canonicalize(path) {
        return Some(real);
    }
    // The leaf, or a whole trailing chain, may be gone — a stale history row.
    // Resolve through the nearest ancestor that does exist and re-attach the
    // rest verbatim, so `..` and symlinks above that point are still resolved
    // and a relative path with no real base resolves to `None`.
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    let mut current = path;
    loop {
        missing.push(current.file_name()?.to_os_string());
        let parent = current.parent()?;
        if let Ok(real) = std::fs::canonicalize(parent) {
            let mut resolved = real;
            for part in missing.iter().rev() {
                resolved.push(part);
            }
            return Some(resolved);
        }
        if parent.as_os_str().is_empty() {
            return None;
        }
        current = parent;
    }
}

/// Push `candidate` onto `roots`, resolving a relative value against
/// `relative_to` (the way PixivFlow resolves a relative `storage` entry).
fn push_root(roots: &mut Vec<PathBuf>, candidate: &str, relative_to: Option<&Path>) {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return;
    }
    let path = Path::new(trimmed);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(base) = relative_to {
        base.join(path)
    } else {
        path.to_path_buf()
    };
    // A root must exist: a configured directory that is not there cannot be a
    // place this installation downloads to, and admitting a guess would let a
    // typo in the config widen what the host is willing to open.
    if let Ok(real) = std::fs::canonicalize(&absolute) {
        if !roots.contains(&real) {
            roots.push(real);
        }
    }
}

/// Read a `storage.<key>` string out of a PixivFlow config document.
fn storage_path(document: &serde_json::Value, key: &str) -> Option<String> {
    document
        .get("storage")?
        .get(key)?
        .as_str()
        .map(|value| value.to_string())
}

/// The JSON config documents in `dir`, sorted for a stable result.
fn json_documents(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut documents: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().map(|e| e == "json").unwrap_or(false))
        .collect();
    documents.sort();
    documents
}

/// Where a desktop-launched PixivFlow keeps its user-level configuration.
///
/// The backend is spawned with its working directory set to the data root, and
/// its own config discovery *also* looks in `$HOME/.pixivflow/config` — the
/// location anyone who used PixivFlow before the desktop existed still has, and
/// where a WebUI config edit is written. A download directory configured there
/// as an absolute path is just as real as one in the data root, so it has to be
/// enumerated here too; a relative one resolves against the working directory,
/// i.e. the data root.
fn user_config_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|value| !value.is_empty())?;
    Some(PathBuf::from(home).join(".pixivflow").join("config"))
}

/// Add the `storage` directories named by every readable config document.
///
/// A document that cannot be read or parsed contributes nothing at all — the
/// rules widen only from configuration that is really there.
fn push_document_roots(
    documents: Vec<PathBuf>,
    base: Option<&Path>,
    roots: &mut Vec<PathBuf>,
) {
    for document_path in documents {
        let Ok(text) = std::fs::read_to_string(&document_path) else {
            continue;
        };
        let Ok(document) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        for key in [
            "downloadDirectory",
            "illustrationDirectory",
            "novelDirectory",
        ] {
            if let Some(value) = storage_path(&document, key) {
                push_root(roots, &value, base);
            }
        }
    }
}

/// Every directory this installation is allowed to reveal from.
///
/// Four sources, all read-only:
///  - the backend data root and its default `downloads/` directory;
///  - `downloadDir` from the desktop's own config, when the user set one;
///  - the `storage` directories of every PixivFlow config document in
///    `<data root>/config` and `<data root>`;
///  - the same `storage` keys in the user-level config documents
///    (`$HOME/.pixivflow/config/*.json`), which is where a WebUI config edit
///    lands (a user who moved downloads to `/Volumes/Photos` must still be able
///    to reveal them).
///
/// Anything else — the home directory, a system path, another user's data — is
/// simply not a root, so it can never be revealed.
pub fn allowed_roots(data_root: Option<&Path>, desktop_config: &Path) -> Vec<PathBuf> {
    allowed_roots_in(data_root, desktop_config, user_config_dir().as_deref())
}

/// [`allowed_roots`] with the user-level config directory supplied by the
/// caller, so the rules can be tested without reading the real `$HOME`.
fn allowed_roots_in(
    data_root: Option<&Path>,
    desktop_config: &Path,
    user_config: Option<&Path>,
) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Some(root) = data_root {
        // The data root itself is deliberately *not* a root: it also holds the
        // database and the configuration. Only the download directories inside
        // it are enumerated.
        push_root(&mut roots, &root.join("downloads").to_string_lossy(), None);

        // PixivFlow config documents (`<data root>/config/*.json` and a
        // top-level one) own the real download directories.
        let mut documents = json_documents(&root.join("config"));
        documents.extend(json_documents(root));
        push_document_roots(documents, Some(root), &mut roots);
    }

    // The user-level config location, where a WebUI edit lands when PixivFlow
    // was installed before the desktop existed. A relative value there resolves
    // against the backend's working directory — the data root — exactly like
    // the data-root documents; without a data root the document's own directory
    // is the only honest base.
    if let Some(user_config) = user_config {
        let base = data_root.or_else(|| user_config.parent());
        push_document_roots(json_documents(user_config), base, &mut roots);
    }

    if let Ok(text) = std::fs::read_to_string(desktop_config) {
        if let Ok(document) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(value) = document.get("downloadDir").and_then(|v| v.as_str()) {
                push_root(&mut roots, value, desktop_config.parent());
            }
        }
    }

    roots
}

/// Confine `path` to `roots`, or answer `None`.
///
/// Component-wise containment (`Path::starts_with`) is what makes
/// `/downloads-out` a sibling rather than a child of `/downloads`.
pub fn confine(path: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    if path.is_empty() || path.contains('\0') {
        return None;
    }
    let candidate = canonical_candidate(Path::new(path))?;
    for root in roots {
        let Some(root) = canonical_candidate(root) else {
            continue;
        };
        if candidate == root || candidate.starts_with(&root) {
            return Some(candidate);
        }
    }
    None
}

/// What to hand the file manager for a confined path.
///
/// A file that vanished behind the app's back still has a folder worth showing;
/// only a path whose directory is gone too is a real "not found".
pub fn reveal_target(confined: &Path) -> Result<(PathBuf, bool), String> {
    match std::fs::metadata(confined) {
        Ok(meta) if meta.is_dir() => Ok((confined.to_path_buf(), true)),
        Ok(_) => Ok((confined.to_path_buf(), false)),
        Err(_) => {
            let parent = confined.parent().map(|p| p.to_path_buf());
            match parent {
                Some(parent) if parent.is_dir() => Ok((parent, true)),
                _ => Err(error(
                    NOT_FOUND,
                    "the file and the folder that held it are both gone",
                )),
            }
        }
    }
}

/// Confine a requested path, then show it in the platform file manager.
pub fn reveal(data_root: Option<&Path>, desktop_config: &Path, path: &str) -> Result<(), String> {
    let roots = allowed_roots(data_root, desktop_config);
    let confined = confine(path, &roots).ok_or_else(|| {
        error(
            FORBIDDEN,
            "the path is outside the PixivFlow download directories",
        )
    })?;

    let (target, is_directory) = reveal_target(&confined)?;
    let (program, args) = invocation(
        current_platform(),
        &target.to_string_lossy(),
        is_directory,
    );

    let status = Command::new(&program).args(&args).status().map_err(|e| {
        error(
            UNAVAILABLE,
            &format!("no file manager could be started ({program}): {e}"),
        )
    })?;

    if status.success() {
        Ok(())
    } else {
        Err(error(
            FAILED,
            &format!("{program} exited with {status}"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pfx-reveal-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn macos_selects_a_file_and_opens_a_directory() {
        let (program, args) = invocation(Platform::Macos, "/tmp/a.png", false);
        assert_eq!(program, "open");
        assert_eq!(args, vec!["-R".to_string(), "/tmp/a.png".to_string()]);

        let (program, args) = invocation(Platform::Macos, "/tmp/folder", true);
        assert_eq!(program, "open");
        assert_eq!(args, vec!["/tmp/folder".to_string()]);
    }

    #[test]
    fn windows_selects_a_file_with_explorer() {
        let (program, args) = invocation(Platform::Windows, r"C:\x\a.png", false);
        assert_eq!(program, "explorer");
        assert_eq!(args, vec![r"/select,C:\x\a.png".to_string()]);

        let (program, args) = invocation(Platform::Windows, r"C:\x", true);
        assert_eq!(program, "explorer");
        assert_eq!(args, vec![r"C:\x".to_string()]);
    }

    #[test]
    fn linux_falls_back_to_the_parent_directory() {
        let (program, args) = invocation(Platform::Linux, "/tmp/x/a.png", false);
        assert_eq!(program, "xdg-open");
        assert_eq!(args, vec!["/tmp/x".to_string()]);

        let (program, args) = invocation(Platform::Linux, "/tmp/x", true);
        assert_eq!(program, "xdg-open");
        assert_eq!(args, vec!["/tmp/x".to_string()]);
    }

    #[test]
    fn confines_to_the_allowed_roots() {
        let root = scratch("confine");
        let downloads = root.join("downloads");
        std::fs::create_dir_all(downloads.join("illustrations")).unwrap();
        // A home-directory secret, i.e. what the command must never open.
        let outside = scratch("confine-outside");
        std::fs::create_dir_all(outside.join(".ssh")).unwrap();
        std::fs::write(outside.join(".ssh/id_rsa"), "secret").unwrap();
        std::fs::write(downloads.join("illustrations/a.png"), "png").unwrap();
        // A sibling whose name merely starts with the root's name.
        let sibling = root.join("downloads-out");
        std::fs::create_dir_all(&sibling).unwrap();
        std::fs::write(sibling.join("b.png"), "png").unwrap();

        let roots = allowed_roots_in(Some(&root), &root.join("desktop-config.json"), None);

        let inside = downloads.join("illustrations/a.png");
        assert_eq!(
            confine(&inside.to_string_lossy(), &roots),
            Some(std::fs::canonicalize(&inside).unwrap())
        );

        for refused in [
            outside.join(".ssh/id_rsa"),
            sibling.join("b.png"),
            root.parent().unwrap().join("elsewhere.png"),
        ] {
            assert!(
                confine(&refused.to_string_lossy(), &roots).is_none(),
                "must refuse {}",
                refused.display()
            );
        }

        assert!(confine("", &roots).is_none());
        assert!(confine("/tmp/\0evil", &roots).is_none());
        assert!(confine("relative/no/base.png", &roots).is_none());

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn still_confines_a_file_that_no_longer_exists() {
        let root = scratch("missing");
        let downloads = root.join("downloads");
        std::fs::create_dir_all(downloads.join("illustrations")).unwrap();
        let roots = allowed_roots_in(Some(&root), &root.join("desktop-config.json"), None);

        // The leaf is gone: still confined, so the folder can be shown.
        let gone = downloads.join("illustrations/deleted.png");
        assert!(confine(&gone.to_string_lossy(), &roots).is_some());

        // A whole trailing chain is gone: the nearest existing ancestor decides.
        let deeper = downloads.join("illustrations/gone/child.png");
        assert_eq!(
            confine(&deeper.to_string_lossy(), &roots),
            Some(
                std::fs::canonicalize(downloads.join("illustrations"))
                    .unwrap()
                    .join("gone/child.png")
            )
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn allowed_roots_include_every_configured_download_directory() {
        let root = scratch("roots");
        let config_dir = root.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::create_dir_all(root.join("pics")).unwrap();
        std::fs::create_dir_all(root.join("novels")).unwrap();
        std::fs::create_dir_all(root.join("picked")).unwrap();

        std::fs::write(
            config_dir.join("standalone.config.json"),
            serde_json::json!({
                "storage": {
                    "downloadDirectory": "lib",
                    "illustrationDirectory": "pics",
                    "novelDirectory": "/nonexistent-not-a-root",
                }
            })
            .to_string(),
        )
        .unwrap();
        let desktop_config = root.join("desktop-config.json");
        std::fs::write(
            &desktop_config,
            serde_json::json!({ "downloadDir": "picked" }).to_string(),
        )
        .unwrap();

        let roots = allowed_roots_in(Some(&root), &desktop_config, None);
        for expected in ["lib", "pics", "picked"] {
            let dir = std::fs::canonicalize(root.join(expected)).unwrap();
            assert!(roots.contains(&dir), "missing root {}", expected);
        }
        // A directory that does not exist cannot be a root.
        assert!(!roots.iter().any(|r| r.ends_with("nonexistent-not-a-root")));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_download_directory_set_in_the_user_config_is_revealable() {
        // A user who installed PixivFlow before the desktop keeps their config
        // in `$HOME/.pixivflow/config`, and the WebUI writes edits there: an
        // absolute download directory must still be revealable.
        let root = scratch("user-roots");
        let user_config = root.join("home/.pixivflow/config");
        std::fs::create_dir_all(&user_config).unwrap();
        let archive = root.join("Volumes-ish/Archive");
        std::fs::create_dir_all(&archive).unwrap();
        std::fs::write(
            user_config.join("standalone.config.json"),
            serde_json::json!({
                "storage": {
                    "illustrationDirectory": archive.to_string_lossy(),
                }
            })
            .to_string(),
        )
        .unwrap();

        let roots = allowed_roots_in(
            None,
            &root.join("desktop-config.json"),
            Some(&user_config),
        );
        assert!(
            roots.contains(&std::fs::canonicalize(&archive).unwrap()),
            "the user-level config directory must contribute roots: {roots:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_file_is_shown_through_its_directory() {
        let root = scratch("target");
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("kept.png");
        std::fs::write(&file, "png").unwrap();

        assert_eq!(reveal_target(&file).unwrap(), (file.clone(), false));
        assert_eq!(reveal_target(&root).unwrap(), (root.clone(), true));

        // Gone, but its folder is not: show the folder.
        let gone = root.join("deleted.png");
        assert_eq!(reveal_target(&gone).unwrap(), (root.clone(), true));

        // Folder gone too: a real not-found, never a spawn.
        let deeper = root.join("gone/child/deleted.png");
        let err = reveal_target(&deeper).unwrap_err();
        assert!(err.starts_with(NOT_FOUND), "got {err}");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reveal_refuses_outside_the_download_directories_without_spawning() {
        let root = scratch("refuse");
        std::fs::create_dir_all(root.join("downloads")).unwrap();
        let outside = scratch("refuse-outside");
        std::fs::create_dir_all(outside.join(".ssh")).unwrap();
        let secret = outside.join(".ssh/id_rsa");
        std::fs::write(&secret, "secret").unwrap();

        let err = reveal(
            Some(&root),
            &root.join("desktop-config.json"),
            &secret.to_string_lossy(),
        )
        .unwrap_err();
        assert!(err.starts_with(FORBIDDEN), "got {err}");

        // Inside the allowed tree, but nothing to show: also no spawn.
        let err = reveal(
            Some(&root),
            &root.join("desktop-config.json"),
            &root.join("downloads/gone/child.png").to_string_lossy(),
        )
        .unwrap_err();
        assert!(err.starts_with(NOT_FOUND), "got {err}");

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
