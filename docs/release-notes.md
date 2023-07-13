# Release notes

## Upcoming liboverdrop 0.2.0 (unreleased)

Changes:

- Require Rust ≥ 1.64.0
- Add release notes doc

New contributors:



## liboverdrop 0.1.0 (2023-02-03)

Changes:

- Replace FragmentScanner with scan(), and turn around the bool by @nabijaczleweli in https://github.com/coreos/liboverdrop-rs/pull/20
- Don't completely skip files whose basenames aren't UTF-8 by @nabijaczleweli in https://github.com/coreos/liboverdrop-rs/pull/21
- Release 0.1.0, incl. migration guide by @nabijaczleweli in https://github.com/coreos/liboverdrop-rs/pull/24

**Full Changelog**: https://github.com/coreos/liboverdrop-rs/compare/0.0.4...0.1.0


## liboverdrop 0.0.4 (2023-02-03)

This is a semver-compatible release in the 0.0 series, in preparation for merging API bumps for the forthcoming 0.1 (or maybe 0.5 if we're feeling ambitious).

Changes:

- Fix two minor clippy lints by @cgwalters in https://github.com/coreos/liboverdrop-rs/pull/14
- Enable CI, update metadata, and update to Rust 2021 by @bgilbert in https://github.com/coreos/liboverdrop-rs/pull/16
- Directly import `BTreeMap` and `PathBuf` by @cgwalters in https://github.com/coreos/liboverdrop-rs/pull/18
- Drop another allocation in the loop by @cgwalters in https://github.com/coreos/liboverdrop-rs/pull/19
- Don't re-allocate every file's extension by @nabijaczleweli in https://github.com/coreos/liboverdrop-rs/pull/22
- Release 0.0.4 by @cgwalters in https://github.com/coreos/liboverdrop-rs/pull/23

New contributors:

- @cgwalters made their first contribution in https://github.com/coreos/liboverdrop-rs/pull/14
- @bgilbert made their first contribution in https://github.com/coreos/liboverdrop-rs/pull/16
- @nabijaczleweli made their first contribution in https://github.com/coreos/liboverdrop-rs/pull/22


## liboverdrop 0.0.2 (2019-06-25)

Changes:

- docs: expand description


## liboverdrop 0.0.1 (2019-06-14)

Changes:

- Initial release
