# Changelog

All notable changes are documented here.

## [Unreleased]

### Added

- Added `Device::connect` with [configuration](https://example.com/config).

  The continuation accepts arbitrarily wrapped descriptions and Unicode: λ.

  - Nested retry policy.
  - Nested timeout.

  ```rust
  let device = Device::connect(config).await?;
  ```

- Added `Session::close`.

### Changed

- Updated `Api<T>` to retain context.

## [1.0.0] - 2026-10-19

### Security

- Historical release prose must remain byte-identical.

[Unreleased]: https://example.com/compare/v1.0.0...HEAD
[1.0.0]: https://example.com/releases/tag/v1.0.0
