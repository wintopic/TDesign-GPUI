# Changelog

All notable changes are documented here. The project follows semantic
versioning after `1.0`; `0.x` releases may add components and refine APIs.

## [Unreleased]

### Fixed

- Correct Collapse accordion toggling, Input/IME disabled handling, CRLF normalization,
  DatePicker and TimePicker boundary behavior, Watermark visibility, ImageViewer sizing,
  QR error rendering and quiet zones, Table striping, and virtualized row bounds.
- Prevent text selectors from submitting on Space, reject disabled selections consistently,
  suppress unchanged value callbacks, stabilize InputNumber formatting, and keep Form submit
  guards current at click time.
- Stream multipart upload bodies instead of buffering complete files in memory, with
  cancellation checks, transport progress, and generation-safe same-path retries while
  the HTTP client consumes the body.
- Wire built-in and application locale messages into selectors, date/time controls,
  transfers, uploads, dialogs, and confirmation actions with key-name fallback.
- Make dialog and drawer controls dismiss their overlays, stack transient messages and
  notifications, add close affordances, expire them after three seconds, and ensure Escape
  targets interactive overlays before transient notices.
- Apply resolved theme tokens to icons and core button variants while preserving explicit
  Light/Dark token edits and brand overrides.
- Harden CI, release and upstream synchronization with locked dependency resolution,
  package-level minimal-feature builds, clippy, GPUI patch verification, safer SVG parsing,
  pinned action revisions, cold-runner parity regeneration, full locked-snapshot parity
  comparison, generated-branch CI gating, and release tag/version and license validation.
- Clarify the smoke gallery and minimal-icon behavior, fail fast on missing icon assets,
  include legal notices in both published crates, and remove stale generated review reports
  from version control.

## [0.1.0] - 2026-08-12

- Prepare the initial `0.1.0` open-source workspace with professional English and
  Simplified Chinese documentation, a compiled native example, contribution
  guidance, security/support policies, and GitHub issue/PR automation.
- Initial Cargo workspace with 71 React PC component surfaces, native
  `ConfigProvider`, light/dark themes, and 2,354 generated icons.
- Native typed input events and IME-backed editors for Input, Textarea,
  AutoComplete, TagInput, RangeInput, and SelectInput.
- Keyboard/range behavior for Calendar, DatePicker, TimePicker, Select, Tree,
  Transfer, Menu, Tabs, Pagination, Slider, and toggle controls.
- Upload progress, cancellation, retry, multipart HTTP transport, and native
  file selection.
- React(PC)-scoped API parity generation, upstream sync classification, GPUI
  main canary workflow, and crates.io release checks.
- Large List/Table/Tree virtualization smoke coverage and a native gallery.
