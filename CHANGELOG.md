# Changelog

All notable changes to Terrarium are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

- Short windows scroll from the top, and the log wraps under the jar when it doesn't fit beside it.

## [0.2.0] - 2026-10-09

### Changed

- Add a Settings drawer for scheme, appearance, and FPS. Restore saved appearance at launch.
- Include current Ferrite layout, window memory, and overlay fixes.

## [0.1.1] - 2026-10-07

### Fixed

- Maximized or fullscreen, the jar is drawn at 2× or 3× (the largest
  crisp size that fits) and sits centered instead of small in a corner.
- The window reopens at the size, place and state it was closed in, and
  the first one fits the screen.
- "Start a new jar?": the buttons no longer spill out of the dialog at
  high display scaling (fixed in ferrite-design).
- The log's labels are measured in font cells, so they line up at any
  display scaling.

## [0.1.0] - 2026-10-07

The first release.

### Added

- A jar that grows over real days: in daylight on your clock, on damp soil.
- Stems, leaves, a bud and a flower, creeping moss, stones and a snail, all
  drawn in text from the jar's seed.
- Soil that dries over days, watering (<kbd>W</kbd>), wilting and recovery.
- A journal of firsts, and a log with soil and sun gauges.
- The jar replayed from its seed, planting time and waterings, so it carries
  on while the app is closed; `--days N` previews an older jar.
- The Verdigris scheme by default, and the shared look from Lodestone
  (`FERRITE_*`) when launched from it.
- The pixel-art logo as the Windows executable, window and taskbar icon.

[0.1.0]: https://github.com/rwetz/ferrite-terrarium/releases/tag/v0.1.0
