//! Bounded, read-only discovery of local opening type packages.
use crate::{
    MAX_OPENING_TYPE_PACKAGE_BYTES, OpeningTypePackage, parse_opening_type_package, storage,
};
use os_core::Result;
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

pub const MAX_OPENING_TYPE_LIBRARY_ENTRIES: usize = 4096;
pub const MAX_OPENING_TYPE_LIBRARY_PACKAGES: usize = 128;
pub const MAX_OPENING_TYPE_LIBRARY_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct OpeningTypeLibraryEntry {
    pub path: PathBuf,
    pub package: OpeningTypePackage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpeningTypeLibraryIssue {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpeningTypeLibraryScan {
    pub entries: Vec<OpeningTypeLibraryEntry>,
    pub issues: Vec<OpeningTypeLibraryIssue>,
    /// Regular .osot files discovered, including invalid or unreadable packages.
    pub package_files: usize,
    /// Bytes actually read, including bytes from invalid packages.
    /// Oversized files rejected using metadata contribute zero bytes.
    pub bytes_examined: usize,
}

/// Scan immediate regular files with an ASCII case-insensitive `.osot` extension.
/// Symlinks and directories are skipped. Results use native `PathBuf` sort order.
/// Directory enumeration errors and exceeded scan limits fail the whole scan;
/// file read/validation failures are issues and preserve other valid entries.
/// No files are created or changed. A scan is not an atomic directory snapshot.
pub fn scan_opening_type_library(directory: &Path) -> Result<OpeningTypeLibraryScan> {
    let mut scan = OpeningTypeLibraryScan::default();
    let mut paths = Vec::new();
    for (index, entry) in fs::read_dir(directory).map_err(storage)?.enumerate() {
        if index >= MAX_OPENING_TYPE_LIBRARY_ENTRIES {
            return Err(storage(
                "opening type library exceeds 4096 directory entries",
            ));
        }
        let entry = entry.map_err(storage)?;
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("osot"))
        {
            continue;
        }
        match entry.file_type() {
            Ok(kind) if kind.is_file() => paths.push(path),
            Ok(_) => continue,
            Err(error) => scan.issues.push(OpeningTypeLibraryIssue {
                path,
                message: error.to_string(),
            }),
        }
        if paths.len() > MAX_OPENING_TYPE_LIBRARY_PACKAGES {
            return Err(storage("opening type library exceeds 128 package files"));
        }
    }
    paths.sort();
    scan.package_files = paths.len();
    for path in paths {
        // Recheck the path and opened file in case an entry changed after discovery.
        let opened = (|| -> Result<_> {
            if !fs::symlink_metadata(&path).map_err(storage)?.is_file() {
                return Err(storage("package is no longer a regular file"));
            }
            let file = File::open(&path).map_err(storage)?;
            let metadata = file.metadata().map_err(storage)?;
            if !metadata.is_file() {
                return Err(storage("package is no longer a regular file"));
            }
            if metadata.len() > MAX_OPENING_TYPE_PACKAGE_BYTES as u64 {
                return Err(storage("opening type package exceeds 1 MiB limit"));
            }
            Ok((file, metadata.len() as usize))
        })();
        let result = match opened {
            Err(error) => Err(error),
            Ok((mut file, length)) => {
                if length > MAX_OPENING_TYPE_LIBRARY_BYTES - scan.bytes_examined {
                    return Err(storage("opening type library exceeds 32 MiB byte limit"));
                }
                let mut bytes = Vec::new();
                // Bound reads to the observed length, even if the file grows.
                let read = (&mut file).take(length as u64).read_to_end(&mut bytes);
                scan.bytes_examined += bytes.len();
                (|| {
                    read.map_err(storage)?;
                    if bytes.len() != length
                        || file.metadata().map_err(storage)?.len() != length as u64
                    {
                        return Err(storage("package length changed during scan"));
                    }
                    parse_opening_type_package(&bytes)
                })()
            }
        };
        match result {
            Ok(package) => scan.entries.push(OpeningTypeLibraryEntry { path, package }),
            Err(error) => scan.issues.push(OpeningTypeLibraryIssue {
                path,
                message: error.to_string(),
            }),
        }
    }
    scan.issues.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(scan)
}
