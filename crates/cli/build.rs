//! Compile-time source revision receipt for the packaged CLI.
//!
//! `GIT_COMMIT` identifies the source base revision only. It does not represent
//! dirty working-tree content or uncommitted changes.

use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const MAX_FILE_BYTES: usize = 4096;
const MAX_GIT_OUTPUT_BYTES: usize = 128;
const INVALID_REVISION: &str =
    "OC2_BUILD_REVISION must be exactly 40 lowercase hexadecimal characters";

struct RepositoryMetadata {
    root: PathBuf,
    watch_paths: Vec<PathBuf>,
}

fn main() {
    println!("cargo:rerun-if-env-changed=OC2_BUILD_REVISION");

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = find_repository(manifest_dir);

    if let Some(repository) = &repository {
        for path in &repository.watch_paths {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    let revision = match env::var_os("OC2_BUILD_REVISION") {
        Some(value) => match value.to_str() {
            Some(value) if valid_revision(value) => Some(value.to_owned()),
            _ => fail_invalid_revision(),
        },
        None => repository
            .as_ref()
            .and_then(|repository| git_revision(&repository.root)),
    };

    if let Some(revision) = revision {
        println!("cargo:rustc-env=GIT_COMMIT={revision}");
    }
}

fn fail_invalid_revision() -> ! {
    eprintln!("{INVALID_REVISION}");
    std::process::exit(1);
}

fn valid_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn find_repository(manifest_dir: &Path) -> Option<RepositoryMetadata> {
    let mut candidate = Some(manifest_dir);
    while let Some(path) = candidate {
        let marker = path.join(".git");
        if marker.is_dir() {
            return Some(metadata_for_git_dir(path, marker));
        }
        if marker.is_file() {
            let git_dir = git_dir_from_pointer(path, &marker)?;
            return Some(metadata_for_git_dir(path, git_dir));
        }
        candidate = path.parent();
    }
    None
}

fn git_dir_from_pointer(repo_root: &Path, marker: &Path) -> Option<PathBuf> {
    let pointer = read_bounded(marker)?;
    let pointer = std::str::from_utf8(&pointer).ok()?;
    let value = pointer.lines().next()?.strip_prefix("gitdir:")?.trim();
    if value.is_empty() {
        return None;
    }
    let path = Path::new(value);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    })
}

fn metadata_for_git_dir(repo_root: &Path, git_dir: PathBuf) -> RepositoryMetadata {
    let mut metadata = RepositoryMetadata {
        root: repo_root.to_path_buf(),
        watch_paths: Vec::with_capacity(8),
    };
    add_git_metadata(&mut metadata.watch_paths, &git_dir);

    if let Some(common_dir) = common_git_dir(&git_dir) {
        if common_dir != git_dir {
            add_git_metadata(&mut metadata.watch_paths, &common_dir);
            if let Some(reference) = head_reference(&git_dir.join("HEAD")) {
                add_watch_path(&mut metadata.watch_paths, common_dir.join(reference));
            }
        }
    }

    metadata
}

fn add_git_metadata(watch_paths: &mut Vec<PathBuf>, git_dir: &Path) {
    let head = git_dir.join("HEAD");
    add_watch_path(watch_paths, head.clone());
    add_watch_path(watch_paths, git_dir.join("packed-refs"));

    if let Some(reference) = head_reference(&head) {
        add_watch_path(watch_paths, git_dir.join(reference));
    }
}

fn common_git_dir(git_dir: &Path) -> Option<PathBuf> {
    let path = git_dir.join("commondir");
    let bytes = read_bounded(&path)?;
    let value = std::str::from_utf8(&bytes).ok()?.lines().next()?.trim();
    if value.is_empty() {
        return None;
    }
    let path = Path::new(value);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        git_dir.join(path)
    })
}

fn head_reference(head: &Path) -> Option<PathBuf> {
    let bytes = read_bounded(head)?;
    let value = std::str::from_utf8(&bytes)
        .ok()?
        .lines()
        .next()?
        .strip_prefix("ref:")?
        .trim();
    if !value.starts_with("refs/")
        || value.contains('\\')
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    Some(PathBuf::from(value))
}

fn add_watch_path(watch_paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !watch_paths.iter().any(|existing| existing == &path) {
        watch_paths.push(path);
    }
}

fn read_bounded(path: &Path) -> Option<Vec<u8>> {
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= MAX_FILE_BYTES).then_some(bytes)
}

fn git_revision(repository_root: &Path) -> Option<String> {
    let path = env::var_os("PATH");
    let mut command = Command::new("git");
    command
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(repository_root)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0");
    if let Some(path) = path {
        command.env("PATH", path);
    }

    let mut child = command.spawn().ok()?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    let mut bytes = Vec::new();
    let read_result = stdout
        .take((MAX_GIT_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes);
    if read_result.is_err() || bytes.len() > MAX_GIT_OUTPUT_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let status = child.wait().ok()?;
    if !status.success() {
        return None;
    }

    let output = std::str::from_utf8(&bytes).ok()?;
    let mut fields = output.split_whitespace();
    let revision = fields.next()?;
    if fields.next().is_some() || !valid_revision(revision) {
        return None;
    }
    Some(revision.to_owned())
}
