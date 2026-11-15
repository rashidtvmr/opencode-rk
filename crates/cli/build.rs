//! Resolve a bundled dynamic libopentui relative to either CLI executable.
//!
//! The bridge crate selects the native artifact, but its rlib build script
//! cannot set a loader path on the final executable. The installer places the
//! binary in `bin/` and the library in the sibling `lib/` directory.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn main() {
    emit_revision_receipt();

    if std::env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        return;
    }

    let target = std::env::var("TARGET").expect("Cargo supplies TARGET");
    let libdir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../opentui-bridge/native/lib")
        .join(target);
    println!("cargo:rerun-if-changed={}", libdir.display());

    // The bridge prefers a static archive if both forms exist. Static linking
    // needs no runtime lookup; do not add an unnecessary library search path.
    if libdir.join("libopentui.a").is_file() {
        return;
    }

    // Cargo puts the profile directory (`target/<profile>`) on the dynamic
    // loader search path when it runs a test or benchmark binary, but a native
    // library located through `rustc-link-search` is not added there. Without
    // this staging step a test binary that links the bridge dynamically cannot
    // start (`Library not loaded: @rpath/libopentui.dylib`). The installed
    // product is unaffected: it ships `lib/` beside `bin/` and resolves the
    // library through the executable rpath emitted below.
    stage_shared_library(&libdir);

    let rpath = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") if libdir.join("libopentui.dylib").is_file() => "@loader_path/../lib",
        Ok("linux") if libdir.join("libopentui.so").is_file() => "$ORIGIN/../lib",
        _ => return, // The bridge's artifact gate reports unsupported/missing targets.
    };

    // Both oc2 and the compatibility binary are built from this crate. Cargo
    // passes the argument directly to rustc, so `$ORIGIN` is not shell-expanded.
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{rpath}");
}

/// Copy the vendored shared native library beside the build outputs.
///
/// Two consumers need different locations in the build tree:
///
/// * Test/bench binaries run from `<target>/<profile>/deps`; cargo puts the
///   profile directory (and `deps`) on the dynamic loader search path, but the
///   native library lives under `crates/opentui-bridge/native/lib/<triple>` and
///   is only reachable through `rustc-link-search`, which is not part of that
///   path.
/// * `oc2`/`opencode-rk` at `<target>/<profile>/` resolve the library through
///   their `@loader_path/../lib` rpath, which in the build tree is `<target>/lib`
///   (the analogue of the installed `<prefix>/lib` beside `<prefix>/bin`).
///   Tests that spawn those binaries with a sanitized environment rely on it.
///
/// Staging into both directories keeps the installed layout unchanged while
/// letting every workspace target resolve the library. A static archive needs
/// neither copy, so this is only reached for a shared artifact.
fn stage_shared_library(libdir: &Path) {
    let name = match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => "libopentui.dylib",
        Ok("linux") => "libopentui.so",
        Ok("windows") => "opentui.dll",
        _ => return,
    };
    let source = libdir.join(name);
    if !source.is_file() {
        return;
    }
    // `OUT_DIR` is `<target>/<profile>/build/<pkg>-<hash>/out`; three parents
    // up is the profile directory cargo searches when it runs the binary.
    let Some(profile_dir) = std::env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .and_then(|out| out.ancestors().nth(3).map(Path::to_path_buf))
    else {
        return;
    };
    // Best effort: a failure here must not fail the build, because the
    // installed binary resolves the library through its own rpath.
    let _ = std::fs::copy(&source, profile_dir.join(name));
    if let Some(target_dir) = profile_dir.parent() {
        let lib = target_dir.join("lib");
        if std::fs::create_dir_all(&lib).is_ok() {
            let _ = std::fs::copy(&source, lib.join(name));
        }
    }
}

/// Put the source revision into every CLI binary, including non-native builds.
///
/// This is deliberately resolved from the CLI manifest's repository rather
/// than the process cwd: linked worktrees have a `.git` file and may otherwise
/// accidentally inherit the repository containing the caller's shell. An
/// exported source tree can opt in to a receipt with an explicitly supplied,
/// strictly validated `GIT_COMMIT`; it is never assigned a fabricated value.
fn emit_revision_receipt() {
    println!("cargo:rerun-if-env-changed=GIT_COMMIT");
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    // The CLI manifest is `workspace/crates/cli`; source identity belongs to
    // the workspace root, not to an unrelated repository above an export.
    let source_root = manifest_dir.parent().and_then(Path::parent);
    let metadata = source_root.and_then(local_git_metadata);
    if let Some(paths) = source_root
        .and_then(|root| metadata.as_ref().and_then(|_| git_watch_paths(root)))
    {
        for path in paths {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    let explicit = std::env::var("GIT_COMMIT").ok();
    let revision = if let Some(explicit) = explicit.as_deref() {
        valid_sha(Some(explicit))
    } else if metadata.is_some() {
        source_root.and_then(git_revision)
    } else {
        None
    };

    if let Some(revision) = revision {
        println!("cargo:rustc-env=GIT_COMMIT={revision}");
    }
}

fn local_git_metadata(manifest_dir: &Path) -> Option<PathBuf> {
    let dot_git = manifest_dir.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    if dot_git.is_file() {
        let content = std::fs::read_to_string(&dot_git).ok()?;
        let target = content.trim().strip_prefix("gitdir:")?.trim();
        let target = PathBuf::from(target);
        return Some(if target.is_absolute() {
            target
        } else {
            manifest_dir.join(target)
        });
    }
    None
}

fn git_watch_paths(manifest_dir: &Path) -> Option<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for relative in ["HEAD", "packed-refs"] {
        if let Some(path) = git_path(manifest_dir, relative) {
            paths.push(path);
        }
    }
    if let Some(branch) = git_output(manifest_dir, &["symbolic-ref", "--quiet", "--short", "HEAD"])
    {
        if let Some(path) = git_path(manifest_dir, &format!("refs/heads/{branch}")) {
            paths.push(path);
        }
    }
    Some(paths)
}

fn git_path(manifest_dir: &Path, path: &str) -> Option<PathBuf> {
    let output = git_output(manifest_dir, &["rev-parse", "--git-path", path])?;
    let path = PathBuf::from(output);
    Some(if path.is_absolute() {
        path
    } else {
        manifest_dir.join(path)
    })
}

fn git_output(manifest_dir: &Path, args: &[&str]) -> Option<String> {
    let output = sanitized_git(manifest_dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

fn sanitized_git(manifest_dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(manifest_dir)
        .stdin(Stdio::null());
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_NAMESPACE",
        "GIT_CEILING_DIRECTORIES",
        "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    ] {
        command.env_remove(key);
    }
    command
}

fn git_revision(manifest_dir: &Path) -> Option<String> {
    let root = git_output(manifest_dir, &["rev-parse", "--show-toplevel"])?;
    if Path::new(&root) != manifest_dir {
        return None;
    }
    let output = sanitized_git(manifest_dir)
        .args(["rev-parse", "--verify", "HEAD^{commit}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let revision = String::from_utf8(output.stdout).ok()?;
    let revision = revision.trim();
    valid_sha(Some(revision))
}

fn valid_sha(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    (value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_ascii_lowercase())
}
