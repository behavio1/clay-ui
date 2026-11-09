//! File writes that never follow a symbolic link.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

pub fn path_taken(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn describe(error: std::io::Error, path: &Path) -> String {
    if error.raw_os_error() == Some(40) || error.raw_os_error() == Some(libc::ELOOP) {
        return format!(
            "Refusing to write through a symbolic link: {}",
            path.display()
        );
    }
    format!("{error}, open '{}'", path.display())
}

fn nofollow(options: &mut OpenOptions) -> &mut OpenOptions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o644).custom_flags(libc::O_NOFOLLOW)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
    }
}

/// Creates each folder below `base`, refusing symlinks so a write can never leave the target folder.
pub fn ensure_real_folder(base: &Path, parts: &[&str]) -> Result<PathBuf, String> {
    fs::create_dir_all(base).map_err(|error| format!("{error}, mkdir '{}'", base.display()))?;
    let mut current = base.to_path_buf();
    for part in parts.iter().filter(|part| !part.is_empty()) {
        current.push(part);
        if is_link(&current) {
            return Err(format!(
                "Refusing to write through a symbolic link: {}",
                current.display()
            ));
        }
        if !current.exists() {
            fs::create_dir(&current)
                .map_err(|error| format!("{error}, mkdir '{}'", current.display()))?;
        }
    }
    Ok(current)
}

pub fn write_no_follow(file: &Path, content: &[u8]) -> Result<(), String> {
    if cfg!(windows) && is_link(file) {
        return Err(format!(
            "Refusing to write through a symbolic link: {}",
            file.display()
        ));
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let mut handle = nofollow(&mut options)
        .open(file)
        .map_err(|error| describe(error, file))?;
    handle
        .write_all(content)
        .map_err(|error| describe(error, file))
}

/// Temporary file plus rename: readers never see a partial file, and neither step follows a link.
pub fn write_atomic(file: &Path, content: &[u8]) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|error| describe(error, parent))?;
    }
    let mut temporary = file.as_os_str().to_owned();
    temporary.push(format!(".{}.tmp", std::process::id()));
    let temporary = PathBuf::from(temporary);
    let _ = fs::remove_file(&temporary);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut handle = nofollow(&mut options)
        .open(&temporary)
        .map_err(|error| describe(error, &temporary))?;
    handle
        .write_all(content)
        .map_err(|error| describe(error, &temporary))?;
    drop(handle);
    fs::rename(&temporary, file).map_err(|error| describe(error, file))
}

/// Inspect all existing skill path components without creating anything.
pub fn preflight(base: &Path, parts: &[&str], filename: &str) -> Result<(), String> {
    let mut path = base.to_path_buf();
    for part in parts.iter().copied().chain(std::iter::once(filename)) {
        path.push(part);
        if is_link(&path) {
            return Err(format!(
                "Refusing to write through a symbolic link: {}",
                path.display()
            ));
        }
        if part == filename && path.exists() && !path.is_file() {
            return Err(format!(
                "Refusing to write to a non-file: {}",
                path.display()
            ));
        }
        if path.exists() && part != filename && !path.is_dir() {
            return Err(format!("Not a directory: {}", path.display()));
        }
        if let Ok(meta) = fs::metadata(&path) {
            if meta.permissions().readonly() {
                return Err(format!(
                    "Refusing to write to read-only path: {}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}
