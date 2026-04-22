## Unreleased (f8749ab..4369be3)

#### 🐛 Bug Fixes

- (**ci**) use explicit OID range for cog changelog (#30) - (f8749ab) - Zhou Fang

## 0.3.0 - 2026-04-22

#### 🐛 Bug Fixes

- update SharedMedia link, bump MSRV to 1.95, sync version in docs (#22) - (6bee068) - Zhou Fang
- (**ci**) use absolute path for oxfmt in release-prepare - (32bd289) - Zhou Fang

#### 📝 Documentation

- use generic addon names in examples (#21) - (3952025) - Zhou Fang

#### 📦 Build

- migrate to mise for unified toolchain management - (9caf4b2) - Zhou Fang

## 0.2.0 - 2026-04-21

#### ✨ Features

- add configurable max_backups with pruning (#19) - (1437e75) - Zhou Fang

#### 🐛 Bug Fixes

- (**changelog**) restore version history and preserve on future releases (#17) - (183f497) - Zhou Fang

#### 📝 Documentation

- add emoji to changelog section headers (#18) - (16b7fb0) - Zhou Fang

## 0.1.1 - 2026-04-21

#### 🐛 Bug Fixes

- (**ci**) auto-format CHANGELOG and correct set-version flag (#14) - (7f17921) - Zhou Fang

#### ♻️ Refactoring

- decouple library from WindMedia-specific naming (#15) - (c02e3cf) - Zhou Fang

## 0.1.0 - 2026-04-21

#### ✨ Features

- overhaul vendor management, testing, and CI pipeline (#1) - (aa746aa) - Zhou Fang

#### 🐛 Bug Fixes

- (**ci**) skip commit when version unchanged on first release (#12) - (0d75b94) - Zhou Fang
- (**ci**) rewrite release-prepare to handle first release without tags (#11) - (7a8fc94) - Zhou Fang
- (**ci**) add first-release fallback in release-prepare workflow (#10) - (fa93d1b) - Zhou Fang
- (**ci**) route release bumps through a release branch (#8) - (5b9ed01) - Zhou Fang
- (**ci**) correct stylua-action and cocogitto-action usage - (422376b) - Zhou Fang
- (**ci**) install cargo-edit in release workflow and gitignore CHANGELOG.md - (23ad336) - Zhou Fang
- (**ci**) fix release workflow cocogitto-action version and first-release support (#4) - (bfb1d1c) - Zhou Fang

#### 📦 Build

- pin vendor snapshots and tighten release publishing (#3) - (26a2ac6) - Zhou Fang

#### 👷 CI

- use stylua-action and add emoji step names - (3301ddd) - Zhou Fang

#### ♻️ Refactoring

- rename crate from wow-windmedia to wow-sharedmedia (#9) - (5033957) - Zhou Fang
