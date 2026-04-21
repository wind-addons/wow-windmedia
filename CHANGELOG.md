## 0.1.0 - 2026-04-21

#### Features

- overhaul vendor management, testing, and CI pipeline (#1) - (aa746aa) - Zhou Fang

#### Bug Fixes

- (**ci**) skip commit when version unchanged on first release (#12) - (0d75b94) - Zhou Fang
- (**ci**) rewrite release-prepare to handle first release without tags (#11) - (7a8fc94) - Zhou Fang
- (**ci**) add first-release fallback in release-prepare workflow (#10) - (fa93d1b) - Zhou Fang
- (**ci**) route release bumps through a release branch (#8) - (5b9ed01) - Zhou Fang
- (**ci**) correct stylua-action and cocogitto-action usage - (422376b) - Zhou Fang
- (**ci**) install cargo-edit in release workflow and gitignore CHANGELOG.md - (23ad336) - Zhou Fang
- (**ci**) fix release workflow cocogitto-action version and first-release support (#4) - (bfb1d1c) - Zhou Fang

#### Build

- pin vendor snapshots and tighten release publishing (#3) - (26a2ac6) - Zhou Fang

#### CI

- use stylua-action and add emoji step names - (3301ddd) - Zhou Fang

#### Refactoring

- rename crate from wow-windmedia to wow-sharedmedia (#9) - (5033957) - Zhou Fang
