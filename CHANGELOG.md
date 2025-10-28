# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate


## [0.2.1] - 2025-10-28

Patching missing updates from [0.2.0].

### Added
- `new_from()` method for transforming one ArenaBox into another while reusing the arena

### Changed
- **BREAKING:** `make_arena_version!` macro now takes visibility modifier for the alias, not the original type
  - Before: `make_arena_version!(pub Data, ArenaData)`
  - After: `make_arena_version!(Data, pub ArenaData)`

## [0.1.0] - 2025-01-28

### Added
- Initial release of `arena-box`
- `ArenaBox<T>` smart pointer for self-referential structs
- `WithLifetime` trait with Generic Associated Types (GATs)
- `make_arena_version!` macro for convenient type creation
- `MutHandle` for safe mutation with arena access
- `new()` method for constructing arena-boxed values
- `get()` method for immutable access
- `mutate()` method returning a `MutHandle` for mutation
- `Display`, `Debug`, and `PartialEq` trait implementations
- `no_std` support (requires `alloc`)
- Comprehensive documentation and examples
- Error handling use case with context accumulation

<!-- next-url -->
[Unreleased]: https://github.com/NiltonVolpato/arena-box/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/NiltonVolpato/arena-box/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/NiltonVolpato/arena-box/releases/tag/v0.2.0
[0.1.0]: https://github.com/NiltonVolpato/arena-box/releases/tag/v0.1.0
