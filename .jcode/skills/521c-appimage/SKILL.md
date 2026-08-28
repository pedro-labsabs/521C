---
name: 521c-appimage
description: Use when packaging 521C for distribution, such as producing or debugging the AppImage, desktop entry, AppStream metadata, or the CI packaging job. Grounds the agent in the repository's packaging layout and toolchain rules.
allowed-tools: bash, read, write, edit, apply_patch, agentgrep, todo
---

# 521C AppImage packaging

The primary release artifact for 521C is an AppImage, with standard desktop
metadata.

## Layout

- `scripts/package-appimage.sh` — builds `native/dist/521C-<version>-x86_64.AppImage`
  from the release build of the 521c-desktop crate plus `packaging/linux`
  metadata and crate icon assets.
- `packaging/linux/521c.desktop` — desktop entry.
- `packaging/linux/io.github.pedro-labsabs.521c.metainfo.xml` — AppStream
  metadata.
- `native/crates/521c-desktop/assets/` — icons: SVG plus PNG sizes
  (16/32/48/64/128/256/512).
- CI job `Desktop · AppImage artifact` in `.github/workflows/ci.yml` builds
  the artifact for every PR (it is one of the required checks on main).

## Tooling rules

- `appimagetool` resolution order: `$APPIMAGETOOL`, PATH,
  `~/.cache/521c-tools`. The tool is official AppImage project release tooling
  and is NOT vendored in the repository. Do not add third-party APT repos,
  PPAs or unsigned sources just to get packaging tools (see
  `docs/HOST_SAFETY.md`).
- Keep normal runtime non-root and local-first; packaging must not weaken
  runtime safety (no telemetry/implicit network).

## Workflow hints

- After changing the desktop crate, metadata, or icons, run
  `scripts/package-appimage.sh` and smoke-test the artifact (launch, close via
  `scripts/test-desktop-close.sh`).
- Verify the AppImage filename/version parse from the crate `Cargo.toml`;
  keep `packaging/linux` consistent with the crate id
  `io.github.pedro-labsabs.521c`.
- Document installation/removal for Linux Mint in docs per
  `docs/AUTONOMOUS_EXECUTION.md` release criteria.