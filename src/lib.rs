use log::trace;
use std::{collections, fs, path};

pub struct FragmentScanner {
    dirs: Vec<path::PathBuf>,
    ignore_dotfiles: bool,
    allowed_extensions: Vec<String>,
}

impl FragmentScanner {
    /// Returns a new FragmentScanner, initialized with a vector of directory paths to scan for
    /// configuration fragments.
    ///
    /// # Arguments
    ///
    /// * `base_dirs` - Vector holding base components of directories where configuration fragments
    ///                 are located.
    /// * `shared_path` - Common relative path from each entry in `base_dirs` to the directory
    ///                   holding configuration fragments.
    /// * `ignore_dotfiles` - Whether to ignore dotfiles (hidden files with name prefixed with
    ///                       '.').
    /// * `allowed_extensions` - Only scan files that have an extension listed in
    ///                          `allowed_extensions`. If an empty vector is passed, then any
    ///                          extensions are allowed.
    ///
    /// `shared_path` is concatenated to each entry in `base_dirs` to form the directory paths to
    /// scan.
    ///
    /// # Example
    ///
    /// ```
    /// let base_dirs = vec![
    ///     String::from("/usr/lib"),
    ///     String::from("/run"),
    ///     String::from("/etc"),
    /// ];
    /// let allowed_extensions = vec![
    ///     String::from("toml"),
    /// ];
    ///
    /// use liboverdrop::FragmentScanner;
    /// let od_cfg = FragmentScanner::new(base_dirs, "my-crate/config.d", false, allowed_extensions);
    /// ```
    pub fn new(
        base_dirs: Vec<String>,
        shared_path: &str,
        ignore_dotfiles: bool,
        allowed_extensions: Vec<String>,
    ) -> Self {
        let mut dirs = Vec::with_capacity(base_dirs.len());
        for bdir in base_dirs {
            let mut dpath = path::PathBuf::from(bdir);
            dpath.push(shared_path.clone());
            dirs.push(dpath);
        }
        Self { dirs, ignore_dotfiles, allowed_extensions }
    }

    /// Scan unique configuration fragments from the set configuration directories. Returns a
    /// `collections::BTreeMap` indexed by configuration fragment filename, holding the path where
    /// the unique configuration fragment is located.
    ///
    /// Configuration fragments are stored in the `BTreeMap` in alphanumeric order by filename.
    /// Configuration fragments existing in directories that are scanned later override fragments
    /// of the same filename in directories that are scanned earlier.
    ///
    /// # Example
    ///
    /// ```
    /// let base_dirs = vec![
    ///     String::from("/usr/lib"),
    ///     String::from("/run"),
    ///     String::from("/etc"),
    /// ];
    /// let allowed_extensions = vec![
    ///     String::from("toml"),
    /// ];
    ///
    /// use liboverdrop::FragmentScanner;
    /// let od_cfg = FragmentScanner::new(base_dirs, "my-crate/config.d", false, allowed_extensions);
    /// let fragments = od_cfg.scan();
    /// ```
    pub fn scan(
        &self,
    ) -> collections::BTreeMap<String, path::PathBuf> {
        let mut files_map = collections::BTreeMap::new();
        for dir in &self.dirs {
            trace!("Scanning directory '{}'", dir.display());

            let dir_iter = match fs::read_dir(dir) {
                Ok(iter) => iter,
                _ => continue,
            };
            for dir_entry in dir_iter {
                let entry = match dir_entry {
                    Ok(f) => f,
                    _ => continue,
                };
                let fpath = entry.path();
                let fname = match entry.file_name().into_string() {
                    Ok(n) => n,
                    _ => continue,
                };

                // If hidden files not allowed, ignore dotfiles.
                if self.ignore_dotfiles && fname.starts_with('.') {
                    continue;
                };

                // If extensions are specified, proceed only if filename has one of the allowed
                // extensions.
                if !self.allowed_extensions.is_empty() {
                    if let Some(extension) = fpath.extension() {
                        if let Ok(extension) = &extension.to_owned().into_string() {
                            if !self.allowed_extensions.contains(&extension) {
                                continue;
                            }
                        } else {
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
                        if target == path::PathBuf::from("/dev/null") {
                            trace!("Nulled config file '{}'", fpath.display());
                            files_map.remove(&fname);
                        }
                    }
                    continue;
                }

                // TODO(lucab): return something smarter than a PathBuf.
                trace!("Found config file '{}' at '{}'", fname, fpath.display());
                files_map.insert(fname, fpath);
            }
        }

        files_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FragmentNamePath {
        name: String,
        path: String,
    }

    fn assert_fragments_match(
        fragments: &collections::BTreeMap<String, path::PathBuf>,
        filename: &String,
        filepath: &String,
    ) -> () {
        assert_eq!(fragments.get(filename).unwrap(), &path::PathBuf::from(filepath));
    }

    fn assert_fragments_hit(
        fragments: &collections::BTreeMap<String, path::PathBuf>,
        filename: &str,
    ) -> () {
        assert!(fragments.get(&String::from(filename)).is_some());
    }

    fn assert_fragments_miss(
        fragments: &collections::BTreeMap<String, path::PathBuf>,
        filename: &str,
    ) -> () {
        assert!(fragments.get(&String::from(filename)).is_none());
    }

    #[test]
    fn basic_override() {
        let treedir = "tests/fixtures/tree-basic";
        let dirs = vec![
            format!("{}/{}", treedir, "usr/lib"),
            format!("{}/{}", treedir, "run"),
            format!("{}/{}", treedir, "etc"),
        ];
        let allowed_extensions = vec![
            String::from("toml"),
        ];
        let od_cfg = FragmentScanner::new(dirs, "liboverdrop.d", false, allowed_extensions);

        let expected_fragments = vec![
            FragmentNamePath { name: String::from("01-config-a.toml"), path: treedir.to_owned() + "/etc/liboverdrop.d/01-config-a.toml" },
            FragmentNamePath { name: String::from("02-config-b.toml"), path: treedir.to_owned() + "/run/liboverdrop.d/02-config-b.toml" },
            FragmentNamePath { name: String::from("03-config-c.toml"), path: treedir.to_owned() + "/etc/liboverdrop.d/03-config-c.toml" },
            FragmentNamePath { name: String::from("04-config-d.toml"), path: treedir.to_owned() + "/usr/lib/liboverdrop.d/04-config-d.toml" },
            FragmentNamePath { name: String::from("05-config-e.toml"), path: treedir.to_owned() + "/etc/liboverdrop.d/05-config-e.toml" },
            FragmentNamePath { name: String::from("06-config-f.toml"), path: treedir.to_owned() + "/run/liboverdrop.d/06-config-f.toml" },
            FragmentNamePath { name: String::from("07-config-g.toml"), path: treedir.to_owned() + "/etc/liboverdrop.d/07-config-g.toml" },
        ];

        let fragments = od_cfg.scan();

        for frag in &expected_fragments {
            assert_fragments_match(&fragments, &frag.name, &frag.path);
        }

        // Check keys are stored in the correct order.
        let expected_keys: Vec<String> = expected_fragments.into_iter().map(|x| x.name).collect();
        let fragments_keys: Vec<String> = fragments.keys().cloned().collect();
        assert_eq!(fragments_keys, expected_keys);
    }

    #[test]
    fn basic_override_restrict_extensions() {
        let treedir = "tests/fixtures/tree-basic";
        let dirs = vec![
            format!("{}/{}", treedir, "etc"),
        ];
        let allowed_extensions = vec![
            String::from("toml"),
        ];
        let od_cfg = FragmentScanner::new(dirs, "liboverdrop.d", false, allowed_extensions);

        let fragments = od_cfg.scan();

        assert_fragments_hit(&fragments, "01-config-a.toml");
        assert_fragments_miss(&fragments, "08-config-h.conf");
        assert_fragments_miss(&fragments, "noextension");
    }

    #[test]
    fn basic_override_allow_all_extensions() {
        let treedir = "tests/fixtures/tree-basic";
        let dirs = vec![
            format!("{}/{}", treedir, "etc"),
        ];
        let allowed_extensions = vec![];
        let od_cfg = FragmentScanner::new(dirs, "liboverdrop.d", false, allowed_extensions);

        let fragments = od_cfg.scan();

        assert_fragments_hit(&fragments, "01-config-a.toml");
        assert_fragments_hit(&fragments, "config.conf");
        assert_fragments_hit(&fragments, "noextension");
    }

    #[test]
    fn basic_override_ignore_hidden() {
        let treedir = "tests/fixtures/tree-basic";
        let dirs = vec![
            format!("{}/{}", treedir, "etc"),
        ];
        let allowed_extensions = vec![];
        let od_cfg = FragmentScanner::new(dirs, "liboverdrop.d", true, allowed_extensions);

        let fragments = od_cfg.scan();

        assert_fragments_hit(&fragments, "config.conf");
        assert_fragments_miss(&fragments, ".hidden.conf");
    }

    #[test]
    fn basic_override_allow_hidden() {
        let treedir = "tests/fixtures/tree-basic";
        let dirs = vec![
            format!("{}/{}", treedir, "etc"),
        ];
        let allowed_extensions = vec![];
        let od_cfg = FragmentScanner::new(dirs, "liboverdrop.d", false, allowed_extensions);

        let fragments = od_cfg.scan();

        assert_fragments_hit(&fragments, "config.conf");
        assert_fragments_hit(&fragments, ".hidden.conf");
    }
}
