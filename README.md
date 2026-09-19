# Andromeda Client

Andromeda Client is a native Rust desktop launcher for legitimate Minecraft accounts. The project is being developed incrementally with reliability, secure credential handling, metadata-driven version support, and isolated instances as core requirements.

> **Current status: Milestone 3 download engine.** The reusable engine can securely transfer and verify planned artifacts, but installation planning, authentication, and game launch are not implemented yet.

## Architecture

- Pure Rust desktop UI with Iced
- Testable library plus a thin executable
- Versioned, recoverable settings storage
- Structured local logging with secret-safe value wrappers
- Typed application state and bounded-rate task progress
- Platform paths isolated behind `AppPaths`
- Reusable concurrent download manager with safe resume, retries, cancellation, integrity checks, and atomic publication

See [the complete project blueprint](docs/architecture/project-blueprint.md) and [architecture decisions](docs/decisions/).

Minecraft versions will be sourced from Mojang's official live version manifest. The eventual catalog refresh mechanism will automatically expose newly published versions; versions are not hard-coded. Historical support will be fixture-tested as implementation proceeds, with unsupported metadata reported explicitly rather than launched incorrectly.

## Prerequisites

- Rust 1.88 (automatically selected by `rust-toolchain.toml`)
- Windows 10/11 for the initial supported product target
- On Linux development hosts, the native packages required by Iced/winit

## Build and run

```bash
cargo run
```

The first run creates configuration and logs under platform-appropriate per-user directories. No administrator privileges are required.

## Quality checks

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all-targets
```

CI runs these checks on Windows and Linux. Core tests require neither a GUI, network connection, nor account credentials.

See [Milestone 2 metadata details](docs/milestone-2.md) and [Milestone 3 download-engine details](docs/milestone-3.md).

## Milestone 1 demonstration

- Navigate among Home, Instances, Downloads, Accounts, Logs, and Settings.
- Start the progress demonstration from Home or Downloads.
- Cancel it without blocking the UI.
- Change the theme and snapshot preference; settings persist atomically.
- If settings JSON is corrupt, it is quarantined and the UI reports recovery.

## Security and scope

The launcher will use official Microsoft/Minecraft authentication and legitimate resources only. It will not implement authentication or ownership bypasses. Never add credentials, tokens, or real account metadata to fixtures or logs.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
