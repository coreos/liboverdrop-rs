//! Simple library to handle configuration fragments.
//!
//! This crate provides helpers to scan configuration fragments on disk.
//! The goal is to help in writing Linux services which are shipped as part of a [Reproducible OS][reproducible].
//! Its name derives from **over**lays and **drop**ins (base directories and configuration fragments).
//!
//! The main entrypoint is [`scan`](fn.scan.html). It scans
//! for configuration fragments across multiple directories (with increasing priority),
//! following these rules:
//!
//!  * fragments are identified by unique filenames, lexicographically (e.g. `50-default-limits.conf`).
//!  * in case of name duplication, last directory wins (e.g. `/etc/svc/custom.conf` can override `/usr/lib/svc/custom.conf`).
//!  * a fragment symlinked to `/dev/null` is used to ignore any previous fragment with the same filename.
//!
//! [reproducible]: http://0pointer.net/blog/projects/stateless.html
//!
//! # Example
//!
//! ```rust,no_run
//! # use liboverdrop;
//! // Scan for fragments under:
//! //  * /usr/lib/my-crate/config.d/*.toml
//! //  * /run/my-crate/config.d/*.toml
//! //  * /etc/my-crate/config.d/*.toml
//!
//! let base_dirs = [
//!     "/usr/lib",
//!     "/run",
//!     "/etc",
//! ];
//! let fragments = liboverdrop::scan(&base_dirs, "my-crate/config.d", &["toml"], false);
//!
//! for (filename, filepath) in fragments {
//!     println!("fragment '{}' located at '{}'", filename.to_string_lossy(), filepath.display());
//! }
//! ```
//!
//! # Migrating from liboverdrop 0.0.x
//!
//! The signature changed from
//! ```rust,compile_fail
//! # use liboverdrop::FragmentScanner;
//! let base_dirs: Vec<String> = vec![/**/];
//! let shared_path: &str = "config.d";
//! let allowed_extensions: Vec<String> = vec![/**/];
//! for (basename, filepath) in FragmentScanner::new(
//!                                 base_dirs, shared_path, false, allowed_extensions).scan() {
//!     // basename: String
//!     // filepath: PathBuf
//! }
//! ```
//! to
//! ```rust,no_run
//! # use liboverdrop;
//! # /*
//! let base_dirs: IntoIterator<Item = AsRef<Path>> = /* could be anything */;
//! let shared_path: AsRef<Path> = /* ... */;
//! let allowed_extensions: &[AsRef<OsStr>] = &[/* ... */];
//! # */
//! # let base_dirs = [""];
//! # let shared_path = "";
//! # let allowed_extensions = &[""];
//! for (basename, filepath) in liboverdrop::scan(
//!                                 base_dirs, shared_path, allowed_extensions, false) {
//!     // basename: OsString
//!     // filepath: PathBuf
//! }
//! ```
//!
//! When updating, re-consider if you need to allocate any argument now,
//! since they can all be literals or borrowed.

use log::trace;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// The well-known path to the null device used for overrides.
const DEVNULL: &str = "/dev/null";

/// The base search paths conventionally used by systemd and other projects.
///
/// Here, files in `/run` override those in `/etc`, which in turn override
/// those in `/usr/lib`.
///
/// Note that some projects may want to omit `/usr/local`, which may be a distinct writable
/// area from the OS image base.  To do so, one can explicitly filter it out from this set.
pub const SYSTEMD_CONVENTIONAL_BASES: &[&str] = &["/usr/lib", "/usr/local/lib", "/etc", "/run"];

/// Scan unique configuration fragments from the configuration directories specified.
///
/// # Arguments
///
/// * `base_dirs` - Base components of directories where configuration fragments are located.
/// * `shared_path` - Common relative path from each entry in `base_dirs` to the directory
///                   holding configuration fragments.
/// * `allowed_extensions` - Only scan files that have an extension listed in `allowed_extensions`.
///                          If an empty slice is passed, then all extensions are allowed.
/// * `ignore_dotfiles` - Whether to ignore dotfiles (hidden files with name prefixed with '.').
///
/// `shared_path` is joined onto each entry in `base_dirs` to form the directory paths to scan.
///
/// Returns a `BTreeMap` indexed by configuration fragment filename,
/// holding the path where the unique configuration fragment is located.
///
/// Configuration fragments are stored in the `BTreeMap` in alphanumeric order by filename.
/// Configuration fragments existing in directories that are scanned later override fragments
/// of the same filename in directories that are scanned earlier.
pub fn scan<BdS: AsRef<Path>, BdI: IntoIterator<Item = BdS>, Sp: AsRef<Path>, As: AsRef<OsStr>>(
    base_dirs: BdI,
    shared_path: Sp,
    allowed_extensions: &[As],
    ignore_dotfiles: bool,
) -> BTreeMap<OsString, PathBuf> {
    let shared_path = shared_path.as_ref();

    let mut files_map = BTreeMap::new();
    for dir in base_dirs {
        let dir = dir.as_ref().join(shared_path);
        trace!("Scanning directory '{}'", dir.display());

        let dir_iter = match fs::read_dir(dir) {
            Ok(iter) => iter,
            _ => continue,
        };
        for entry in dir_iter.flatten() {
            let fpath = entry.path();
            let fname = entry.file_name();

            // If hidden files not allowed, ignore dotfiles.
            // Rust RFC 900 &c.: there's no way to check if a Path/OsStr starts with a prefix;
            // instead, we check via to_string_lossy(), which will only allocate if the basename wasn't UTF-8,
            // and the lossiness doesn't bother us; https://github.com/rust-lang/rfcs/issues/900
            if ignore_dotfiles && fname.to_string_lossy().starts_with('.') {
                continue;
            }

            // If extensions are specified, proceed only if filename has one of the allowed
            // extensions.
            if !allowed_extensions.is_empty() {
                if let Some(extension) = fpath.extension() {
                    if !allowed_extensions.iter().any(|ae| ae.as_ref() == extension) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            // Check filetype, ignore non-file.
            let meta = match entry.metadata() {
                Ok(m) => m,
                _ => continue,
            };
            if !meta.file_type().is_file() {
                if let Ok(target) = fs::read_link(&fpath) {
                    // A devnull symlink is a special case to ignore previous file-names.
                    if target == Path::new(DEVNULL) {
                        trace!("Nulled config file '{}'", fpath.display());
                        files_map.remove(&fname);
                    }
                }
                continue;
            }

            trace!(
                "Found config file '{}' at '{}'",
                Path::new(&fname).display(),
                fpath.display()
            );
            files_map.insert(fname, fpath);
        }
    }

    files_map
}

/// This API builds on the [`scan`] functionality, but instead of returning
/// the file paths, calls a user-provided merge function.
///
/// At the current time, this is implemented in a simplistic way that just calls [`scan`] internally.  However,
/// in the future this API can be more efficient as it isn't required to retain
/// all found file paths in memory.
pub fn scan_and_merge<BdS, BdI, Sp, As, F, T, E>(
    base_dirs: BdI,
    shared_path: Sp,
    allowed_extensions: &[As],
    ignore_dotfiles: bool,
    mut merge: F,
) -> Result<T, E>
where
    BdS: AsRef<Path>,
    BdI: IntoIterator<Item = BdS>,
    Sp: AsRef<Path>,
    As: AsRef<OsStr>,
    T: Default,
    F: FnMut(T, &OsStr, std::io::BufReader<File>) -> Result<T, E>,
    E: std::error::Error + From<std::io::Error>,
{
    let mut res = T::default();
    for (k, v) in scan(base_dirs, shared_path, allowed_extensions, ignore_dotfiles) {
        let f = File::open(v).map(BufReader::new)?;
        res = merge(res, &k, f)?;
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::BufRead;

    use super::*;

    fn assert_fragments_match(
        fragments: &BTreeMap<OsString, PathBuf>,
        filename: &OsStr,
        filepath: &Path,
    ) {
        assert_eq!(fragments.get(filename).unwrap(), filepath);
    }

    fn assert_fragments_hit<T: AsRef<OsStr>>(fragments: &BTreeMap<OsString, PathBuf>, filename: T) {
        assert!(fragments.get(filename.as_ref()).is_some());
    }

    fn assert_fragments_miss<T: AsRef<OsStr>>(
        fragments: &BTreeMap<OsString, PathBuf>,
        filename: T,
    ) {
        assert!(fragments.get(filename.as_ref()).is_none());
    }

    #[test]
    fn basic_override() {
        let treedir = "tests/fixtures/tree-basic";
        let dirs = [
            format!("{}/{}", treedir, "usr/lib"),
            format!("{}/{}", treedir, "run"),
            format!("{}/{}", treedir, "etc"),
        ];

        let expected_fragments = [
            (
                OsString::from("01-config-a.toml"),
                Path::new(treedir).join("etc/liboverdrop.d/01-config-a.toml"),
            ),
            (
                OsString::from("02-config-b.toml"),
                Path::new(treedir).join("run/liboverdrop.d/02-config-b.toml"),
            ),
            (
                OsString::from("03-config-c.toml"),
                Path::new(treedir).join("etc/liboverdrop.d/03-config-c.toml"),
            ),
            (
                OsString::from("04-config-d.toml"),
                Path::new(treedir).join("usr/lib/liboverdrop.d/04-config-d.toml"),
            ),
            (
                OsString::from("05-config-e.toml"),
                Path::new(treedir).join("etc/liboverdrop.d/05-config-e.toml"),
            ),
            (
                OsString::from("06-config-f.toml"),
                Path::new(treedir).join("run/liboverdrop.d/06-config-f.toml"),
            ),
            (
                OsString::from("07-config-g.toml"),
                Path::new(treedir).join("etc/liboverdrop.d/07-config-g.toml"),
            ),
        ];

        let fragments = scan(&dirs, "liboverdrop.d", &["toml"], false);

        for (name, path) in &expected_fragments {
            assert_fragments_match(&fragments, name, path);
        }

        // Check keys are stored in the correct order.
        let expected_keys: Vec<_> = expected_fragments.into_iter().map(|kv| kv.0).collect();
        let fragments_keys: Vec<_> = fragments.into_iter().map(|kv| kv.0).collect();
        assert_eq!(fragments_keys, expected_keys);
    }

    #[test]
    fn basic_override_systemd() {
        let treedir = Path::new("tests/fixtures/tree-basic");

        let expected_fragments = [
            ("01-config-a.toml", "etc/liboverdrop.d/01-config-a.toml"),
            ("02-config-b.toml", "run/liboverdrop.d/02-config-b.toml"),
            ("03-config-c.toml", "run/liboverdrop.d/03-config-c.toml"),
            ("04-config-d.toml", "usr/lib/liboverdrop.d/04-config-d.toml"),
            ("05-config-e.toml", "etc/liboverdrop.d/05-config-e.toml"),
            ("06-config-f.toml", "run/liboverdrop.d/06-config-f.toml"),
            ("07-config-g.toml", "run/liboverdrop.d/07-config-g.toml"),
        ];

        let dirs = SYSTEMD_CONVENTIONAL_BASES
            .iter()
            .map(|v| treedir.join(v.trim_start_matches('/')));
        let fragments = scan(dirs, "liboverdrop.d", &["toml"], false);

        for (name, path) in &expected_fragments {
            let name = OsStr::new(name);
            let path = treedir.join(path);
            assert_fragments_match(&fragments, name, &path);
        }

        // Check keys are stored in the correct order.
        let expected_keys: Vec<_> = expected_fragments.into_iter().map(|kv| kv.0).collect();
        let fragments_keys: Vec<_> = fragments.into_iter().map(|kv| kv.0).collect();
        assert_eq!(fragments_keys, expected_keys);
    }

    #[test]
    fn basic_override_restrict_extensions() {
        let treedir = "tests/fixtures/tree-basic";
        let dirs = [format!("{}/{}", treedir, "etc")];

        let fragments = scan(dirs, "liboverdrop.d", &["toml"], false);

        assert_fragments_hit(&fragments, "01-config-a.toml");
        assert_fragments_miss(&fragments, "08-config-h.conf");
        assert_fragments_miss(&fragments, "noextension");
    }

    #[test]
    fn basic_override_allow_all_extensions() {
        let treedir = "tests/fixtures/tree-basic";
        let dirs = [format!("{}/{}", treedir, "etc")];

        let fragments = scan::<_, _, _, &str>(&dirs, "liboverdrop.d", &[], false);

        assert_fragments_hit(&fragments, "01-config-a.toml");
        assert_fragments_hit(&fragments, "config.conf");
        assert_fragments_hit(&fragments, "noextension");
    }

    #[test]
    fn basic_override_ignore_hidden() {
        let treedir = "tests/fixtures/tree-basic";
        let dirs = [format!("{}/{}", treedir, "etc")];

        let fragments = scan::<_, _, _, &str>(&dirs, "liboverdrop.d", &[], true);

        assert_fragments_hit(&fragments, "config.conf");
        assert_fragments_miss(&fragments, ".hidden.conf");
    }

    #[test]
    fn basic_override_allow_hidden() {
        let treedir = "tests/fixtures/tree-basic";
        let dirs = [format!("{}/{}", treedir, "etc")];

        let fragments = scan::<_, _, _, &OsStr>(&dirs, "liboverdrop.d", &[], false);

        assert_fragments_hit(&fragments, "config.conf");
        assert_fragments_hit(&fragments, ".hidden.conf");
    }

    type ConfigMap = HashMap<String, String>;

    // Parse a key=value line by line into a HashSet.  In a real world codebase
    // this would be more likely to use e.g. serde to parse a toml or yaml file
    // into a final data structure.
    fn merge_btreemap(
        mut f: ConfigMap,
        name: &OsStr,
        r: BufReader<File>,
    ) -> std::io::Result<ConfigMap> {
        for line in r.lines() {
            let line = line?;
            let (k, v) = line.trim().split_once('=').ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Invalid line {line} in file {name:?}"),
                )
            })?;
            f.insert(k.to_owned(), v.to_owned());
        }
        Ok(f)
    }

    #[test]
    fn basic_merge() {
        let treedir = "tests/fixtures/tree-configmerge";
        let dirs = [
            format!("{}/{}", treedir, "usr/lib"),
            format!("{}/{}", treedir, "etc"),
            format!("{}/{}", treedir, "run"),
        ];

        let config =
            scan_and_merge(&dirs, "liboverdrop.d", &["conf"], true, merge_btreemap).unwrap();

        assert_eq!(config.get("k1").unwrap(), "test1");
        assert_eq!(config.get("k2").unwrap(), "test2");
        assert_eq!(config.get("usrbaseconf").unwrap(), "val01");
        assert_eq!(config.get("usrbaseconf2").unwrap(), "val02");
        assert_eq!(config.get("othermasking").unwrap(), "m2");
        assert_eq!(config.len(), 6);
    }
}
