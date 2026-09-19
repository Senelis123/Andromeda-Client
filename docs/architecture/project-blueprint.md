# Andromeda Client — Project Blueprint

**Status:** Approved; Milestone 2 implemented for review
**Initial target:** Windows 10/11  
**Architecture goal:** Portable core, native desktop shell, official Minecraft services only

## 1. Product overview

Andromeda Client will be a native, primarily Rust Minecraft launcher for legitimate Microsoft/Minecraft accounts. It will install and verify metadata-driven Minecraft versions, manage isolated instances and Java runtimes, launch the game, and present useful progress, logs, and recovery actions through a polished desktop UI.

The product is intentionally not a clone of the official launcher's internals. It consumes documented/public official metadata and authentication endpoints and maintains its own application data. It will not bypass account ownership, authentication, licensing, or entitlement checks.

### Product principles

1. **Correctness before breadth.** Vanilla launch/install/authentication must be reliable before mod loaders.
2. **UI never performs infrastructure work.** It sends commands and renders state/events.
3. **Metadata drives behavior.** Version manifests, rules, arguments, assets, and Java requirements are parsed rather than hard-coded.
4. **Safe, recoverable writes.** Downloads and configuration use staging, validation, and atomic replacement where possible.
5. **Actionable failures.** User messages summarize the failure and recovery; technical context remains available.
6. **Portable core.** Windows integration is isolated behind platform services.
7. **No speculative framework.** Start as one package with a library and binary; split crates only when boundaries or build times justify it.

## 2. Scope and phases

### MVP

- Native Windows desktop shell with Home, Instances, Downloads, Accounts, Settings, and Logs pages.
- Official Microsoft sign-in, Minecraft entitlement/profile lookup, logout, token refresh, and one active account. Data structures support multiple accounts.
- Official global version manifest and per-version metadata retrieval/caching.
- Release versions; snapshot visibility can be enabled because the manifest model supports it.
- Create, edit, duplicate, select, repair, and delete isolated vanilla instances.
- Install/verify client JAR, libraries, natives, asset index/assets, and logging configuration when specified.
- Detect system Java, allow a custom executable, validate architecture/version, and select it per instance.
- Launch modern and legacy metadata formats where practical, with rule evaluation, classpath generation, native extraction, stdout/stderr capture, cancellation/kill, and exit reporting.
- Configurable RAM and additional JVM arguments with guarded validation.
- Concurrent downloads, retry, cancellation, aggregate progress, hash/size verification, and staging.
- Structured rotating file logs plus an in-app game/launcher log view.
- Repair workflows and understandable diagnostics.

### Phase 2

- Multiple accounts and account switching.
- Managed Java runtimes from a trusted, explicitly selected vendor/source.
- Better resumable HTTP downloads, bandwidth/concurrency settings, and download persistence.
- Snapshot UX, import existing vanilla data/instances, instance export/import.
- Per-instance resolution/window options, icons, notes, and richer diagnostics.
- Accessibility audit, keyboard navigation pass, localization foundations.
- Signed Windows installer and automated release pipeline.

### Phase 3

- A loader provider abstraction proven by implementing Fabric first.
- Forge/NeoForge/Quilt providers after format-specific research and tests.
- Mods, resource packs, shader packs, screenshots, backups, and server shortcuts.
- Stable self-update design with signed manifests and rollback.

### Future

- Linux/macOS packaging and credential-store adapters.
- Themes, richer profile sharing, server management, cloud synchronization, and narrowly scoped integrations.
- A plugin system only if real third-party extension requirements emerge; no arbitrary in-process plugin API initially.

### Explicit non-goals for MVP

- Offline/cracked account modes, ownership bypass, mod-loader installation, self-update, cloud sync, embedded browser login, or a general plugin runtime.

## 3. Technology and dependency recommendations

Versions will be pinned after a dependency/security review at implementation time rather than guessed in this blueprint.

### GUI decision

| Candidate | Strengths | Costs/risks | Decision |
|---|---|---|---|
| **Iced** | Pure Rust, Elm-style messages/state, good async model, native renderer, testable update logic, cross-platform | Styling needs deliberate design; ecosystem is younger than web UI | **Recommended** |
| egui/eframe | Excellent iteration speed, mature immediate mode, strong tooling | Application-style navigation/forms and accessibility can feel less native; state patterns can become ad hoc | Good fallback/prototyping option |
| Tauri + web frontend | Best CSS/layout ecosystem and visual polish | Two language/tool chains, IPC boundary, webview variance, larger security surface | Defer unless Iced blocks required UX |

Iced best matches the requested pure-Rust default and enforces a useful state/message architecture. A short Milestone 1 spike must validate text rendering, keyboard focus, scaling, accessibility expectations, long-list performance, and Windows packaging. If that spike fails objective criteria, Tauri is the fallback; changing before domain implementation is inexpensive.

### Core dependencies

| Category | Recommendation | Problem solved / necessity / complexity |
|---|---|---|
| Async | `tokio` | HTTP, filesystem coordination, process I/O and cancellation. Necessary now; use one managed runtime, not runtimes hidden in services. |
| HTTP | `reqwest` with Rustls | Mature async HTTP, streaming, range requests, TLS. Rustls avoids native TLS variance. Redirects, timeouts, and host policies are configured explicitly. |
| Serialization | `serde`, `serde_json` | Official metadata and local JSON. Necessary. Preserve unknown-compatible fields where useful, but reject invalid required fields. |
| Errors | `thiserror`; `anyhow` only at binary/task boundaries | Typed service/domain errors plus context at the outer edge. Avoids stringly typed failures. |
| Logging | `tracing`, `tracing-subscriber`, rolling appender | Structured spans/events and file output. Redaction is mandatory. |
| Cancellation | `tokio-util::sync::CancellationToken` | Cooperative cancellation across install/download work without custom primitives. |
| URLs | `url` | Safe URL parsing/joining; metadata URLs remain untrusted input. |
| Hashing | `sha1`, `sha2`, `hex` | Minecraft metadata commonly supplies SHA-1; SHA-256 supports local/release integrity. Hashes verify integrity, not source authenticity. |
| Paths | `directories` or `directories-next` | Platform-appropriate config/data/cache/log roots without hard-coded Windows paths. |
| IDs/time | `uuid`, `time` | Stable instance/task IDs and serialized timestamps. Limit enabled features. |
| Secrets | `keyring`, behind an internal trait | Windows Credential Manager initially; enables Keychain/Secret Service later. Never put refresh/access tokens in JSON. |
| Atomic config | Small internal temp-write/fsync/rename utility; consider `atomicwrites` only if needed | Keep simple and testable; avoid another dependency until platform semantics demand it. |
| Archive | `zip`, `flate2`/`tar` only when a supported artifact requires them | Native/JRE extraction. All extraction passes through a safe path validator. Do not add formats speculatively. |
| Process discovery | `sysinfo` only if needed | Process/runtime diagnostics. Prefer `std::process`/Tokio process first. |
| Testing | `tempfile`, `wiremock` (or `httpmock`), `proptest` selectively | Isolated filesystems, deterministic HTTP, path/rule property tests. |

### Deliberately deferred dependencies

- **Database:** JSON documents plus cache files are adequate initially. SQLite is reconsidered only for query-heavy mod/catalog data or durable task history.
- **DI framework:** constructors and a small `AppServices` composition root are sufficient.
- **Async trait crate:** stable language features and explicit boxed futures can be assessed when traits are implemented; avoid by default.
- **Full semver parser for Java/Minecraft:** their version schemes are not ordinary SemVer; use dedicated typed parsers.

## 4. System architecture

A pragmatic layered architecture is used within one Rust package:

```text
Iced UI
  │ UserIntent / view state
  ▼
Application layer (use cases, commands, task supervision, event projection)
  │ depends on domain ports
  ▼
Domain/core (models, invariants, rules, plans, typed errors)
  ▲
  │ implementations
Infrastructure (HTTP, files, keyring, process, metadata repositories)
  ▲
Platform adapters (Windows now; Linux/macOS later)
```

Dependency direction is inward: domain code does not import Iced, Reqwest, keyring, or Windows APIs. Not every module needs a trait. Traits are reserved for boundaries that are external, nondeterministic, platform-specific, or useful to fake in tests.

### Runtime and data flow

1. UI emits a typed `UserIntent`.
2. The application update function validates immediate UI state and starts/cancels an application task.
3. A use case calls services and emits throttled `AppEvent`s through a bounded channel.
4. A central event reducer updates canonical `AppState`.
5. Iced renders projections of that state.
6. Services never hold UI handles or display dialogs directly.

Large progress streams are aggregated/throttled (for example 5–10 UI updates/second) to avoid rendering one event per asset. Bounded channels provide backpressure. Task IDs correlate events and make stale-event rejection possible.

## 5. Proposed repository structure

```text
Andromeda-Client/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── rust-toolchain.toml
├── deny.toml
├── .github/workflows/
├── assets/                         # bundled, non-secret app UI assets
├── docs/
│   ├── architecture/project-blueprint.md
│   ├── decisions/                  # short ADRs for accepted decisions
│   ├── development.md
│   ├── security.md
│   └── troubleshooting.md
├── src/
│   ├── main.rs                     # process startup only
│   ├── lib.rs                      # testable library surface
│   ├── bootstrap.rs                # composition root and startup recovery
│   ├── app/
│   │   ├── mod.rs
│   │   ├── state.rs                # canonical application state
│   │   ├── message.rs              # intents/events
│   │   ├── reducer.rs              # state transitions
│   │   ├── task.rs                 # task registry/supervision
│   │   └── use_cases/              # sign-in, install, repair, launch, CRUD
│   ├── domain/
│   │   ├── account.rs
│   │   ├── instance.rs
│   │   ├── java.rs
│   │   ├── version.rs
│   │   ├── download.rs
│   │   ├── launch.rs
│   │   ├── rules.rs
│   │   ├── paths.rs
│   │   └── error.rs
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── service.rs
│   │   ├── microsoft.rs
│   │   ├── xbox.rs
│   │   └── secret_store.rs
│   ├── minecraft/
│   │   ├── mod.rs
│   │   ├── metadata.rs
│   │   ├── repository.rs
│   │   ├── rules.rs
│   │   ├── install.rs
│   │   ├── assets.rs
│   │   ├── libraries.rs
│   │   └── natives.rs
│   ├── downloads/
│   │   ├── mod.rs
│   │   ├── manager.rs
│   │   ├── plan.rs
│   │   ├── transfer.rs
│   │   └── verify.rs
│   ├── java/
│   │   ├── mod.rs
│   │   ├── discovery.rs
│   │   ├── validation.rs
│   │   └── managed.rs              # deferred implementation
│   ├── instances/
│   │   ├── mod.rs
│   │   ├── repository.rs
│   │   └── service.rs
│   ├── launch/
│   │   ├── mod.rs
│   │   ├── planner.rs
│   │   ├── arguments.rs
│   │   ├── classpath.rs
│   │   ├── process.rs
│   │   └── diagnostics.rs
│   ├── config/
│   │   ├── mod.rs
│   │   ├── model.rs
│   │   ├── repository.rs
│   │   └── migration.rs
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── layout.rs
│   │   ├── atomic_file.rs
│   │   └── cache.rs
│   ├── platform/
│   │   ├── mod.rs
│   │   ├── credentials.rs
│   │   ├── process.rs
│   │   ├── shell.rs
│   │   └── windows.rs
│   ├── telemetry/                  # local logging/diagnostics, not remote analytics
│   │   ├── mod.rs
│   │   └── redaction.rs
│   └── ui/
│       ├── mod.rs
│       ├── theme.rs
│       ├── navigation.rs
│       ├── components/
│       └── pages/{home,instances,downloads,accounts,settings,logs}.rs
└── tests/
    ├── fixtures/                   # sanitized pinned metadata samples
    ├── metadata_contract.rs
    ├── install_plan.rs
    ├── launch_plan.rs
    ├── repository_recovery.rs
    └── security_paths.rs
```

This is the target organization, not a requirement to create empty files. Modules are added only as a milestone needs them. A workspace split may later isolate `andromeda-core` from the GUI, but a `lib.rs` already ensures headless testability without early multi-crate overhead.

## 6. Domain models

All persisted models carry `schema_version`. IDs use newtypes rather than interchangeable strings.

### Key models

- `AccountId`, `InstanceId`, `TaskId`, `VersionId`: validated identifiers/newtypes.
- `AccountSummary`: ID, Minecraft UUID/name/skins summary, active state, last successful validation. **No token fields.**
- `AuthSession`: access-token expiry and in-memory status; token material is a secret wrapper whose `Debug` is redacted.
- `Instance`: ID, display name, icon reference, `VersionSpec`, optional loader descriptor, Java selection, memory settings, extra JVM args, game-dir policy, resolution, notes, timestamps.
- `VersionSpec`: vanilla version ID plus optional future `LoaderSpec`. MVP accepts only vanilla.
- `VersionManifest`/`VersionMetadata`: typed representations of official fields including downloads, libraries, rules, arguments, assets, logging, inheritance, main class, and Java component/major version.
- `Artifact`: URL, destination expressed as a validated relative path, expected size, hashes, purpose, and optionality.
- `InstallPlan`: immutable resolved list of artifacts/extractions/directories plus required disk estimate and metadata fingerprint.
- `InstallRecord`: installed version, metadata fingerprint, verified timestamp, and component state. It is an optimization, not the source of truth.
- `JavaRuntime`: executable/canonical home, source (system/custom/managed), vendor, semantic-ish version parts, major version, architecture, capabilities, validation time.
- `JavaRequirement`: minimum/exact major policy, architecture, and optional metadata component name.
- `LaunchPlan`: Java executable, working directory, environment allowlist, JVM arguments (each a separate OS argument), classpath entries, main class, game arguments, redacted printable form.
- `TaskSnapshot`: kind, lifecycle state, progress, cancellability, timestamps, and optional typed failure.
- `Progress`: phase, completed/total bytes and items, smoothed speed, optional ETA, current human-readable item.
- `AppError`: typed category, operation, user summary, technical source/context, retryability, and suggested actions.

### Important invariants

- Instance names are display values; paths derive from stable IDs, not raw names.
- Extra JVM arguments are stored as a parsed argument list, never concatenated into a shell command.
- Download destinations are relative to a preselected root and validated before joining.
- `MemorySettings` enforces positive values and `min <= max`, with practical warnings.
- Version IDs from metadata are treated as untrusted labels; they cannot become arbitrary filesystem paths.

## 7. Application state

```text
AppState
├── route + navigation history
├── startup: Loading | Ready | Degraded(problem)
├── settings (non-secret)
├── accounts: summaries + active ID + auth status
├── catalog: remote/cache status + available versions
├── instances: summaries + selected ID + editor draft
├── runtimes: discovered/validated runtimes
├── tasks: TaskId -> TaskSnapshot
├── game_sessions: InstanceId -> process/log status
├── notifications: bounded queue
└── dialogs: explicit modal state
```

Canonical persistent entities live in repositories; `AppState` is a UI-ready snapshot, not a second database. Editor drafts are separate from saved models so Cancel works correctly. Reducers ignore events whose generation/task ID is stale.

### Major ports/services

- `HttpClient`: constrained request/stream abstraction used by metadata, auth, and downloads; faked in tests.
- `SecretStore`: save/load/delete account secrets through platform credentials.
- `Clock`: deterministic expiry/retry tests.
- `VersionRepository`: cached manifest and version metadata.
- `InstanceRepository`: atomic CRUD and schema migration.
- `SettingsRepository`: load/save/recover non-secret settings.
- `DownloadManager`: execute a validated `DownloadPlan` and report aggregate progress.
- `JavaDiscovery` / `JavaValidator`: find candidates and query `java -version`/properties safely.
- `InstallService`: resolve, estimate, download, extract, verify, and commit installs.
- `LaunchPlanner`: pure-ish transformation from instance+metadata+runtime+account+platform into `LaunchPlan`.
- `ProcessRunner`: spawn without a shell, stream output, kill, and report exit.
- `PlatformServices`: app directories, credential backend, shell/browser open, OS/architecture classifier.

Repository traits are useful because persistence is nondeterministic and migration-sensitive. Small pure helpers remain ordinary functions rather than traits.

## 8. Authentication strategy

### Recommended flow

Use Microsoft OAuth 2.0 **Authorization Code with PKCE** in the system browser, using a loopback redirect when permitted by the registered application. Device Code flow is a fallback if registration/platform constraints make loopback unsuitable. Never embed Microsoft credentials in a launcher-controlled webview.

1. Generate cryptographically random PKCE verifier/challenge and `state`.
2. Open the Microsoft authorization URL in the default browser.
3. Receive the loopback callback, verify exact state, exchange the one-time code with the verifier.
4. Exchange the Microsoft access token through the official Xbox Live user authentication chain.
5. Obtain XSTS authorization for the Minecraft relying party.
6. Obtain a Minecraft services access token.
7. Check Minecraft entitlements/ownership and fetch the Minecraft profile.
8. Store refresh token/token set in the OS credential vault under `AccountId`; persist only non-secret account summary in files.
9. Refresh before expiry with a single-flight per-account lock. On invalid grant, transition to `ReauthenticationRequired` rather than repeatedly retrying.

The exact endpoints, scopes, relying-party values, and app registration requirements must be verified against current Microsoft/Minecraft documentation during the authentication milestone. The launcher requires a project-owned Microsoft application registration/client ID; it must not borrow another launcher's identity.

### Security details

- PKCE verifier/state exist only for the pending flow and are zeroized where practical.
- Callback listener binds only to loopback, accepts one expected callback, has a short timeout, and returns no tokens to browser content.
- Tokens never appear in logs, errors, URLs after callback handling, crash bundles, or `Debug` output.
- Access tokens remain in memory as briefly as practical. Logout removes vault entries and invalidates in-memory sessions.
- Entitlement failures are distinct from network/auth failures and cannot be bypassed.
- Multiple-account capable storage is designed now; multi-account UI may follow in Phase 2.

## 9. Minecraft installation/version pipeline

1. **Refresh catalog:** GET official version manifest with timeout, cache validators (`ETag`/`Last-Modified`) where supported, and retain last known good cache.
2. **Resolve version:** select a manifest entry; fetch and validate its metadata. Resolve inherited metadata defensively if encountered, with cycle/depth limits.
3. **Evaluate rules:** apply OS/architecture/feature rules through one tested rule engine shared by install and launch planning.
4. **Build plan:** resolve client, logging config, libraries, classifiers/natives, asset index and assets into validated destinations. Deduplicate by destination+hash and reject conflicts.
5. **Preflight:** calculate missing bytes plus staging/extraction margin, check writable roots/disk availability, Java requirement, cancellation, and active-instance conflicts.
6. **Reuse:** size/hash-check existing content-addressed assets and libraries. Metadata records accelerate checks but do not replace validation during Repair.
7. **Download:** stream to same-volume `.part` staging files, with bounded concurrency, retry/backoff, cancellation, and progress aggregation.
8. **Verify:** enforce declared size/hash before rename. If no official hash exists, use source HTTPS plus size where available and record the limitation.
9. **Extract natives:** extract only required entries into a fresh version/instance-scoped directory using safe path checks; reject absolute, parent, reserved/device, and symlink-like entries.
10. **Commit:** atomically replace final artifacts where possible, then write install record last.
11. **Repair:** rebuild the plan and verify all required artifacts; redownload only absent/corrupt content.
12. **Remove:** delete only launcher-owned version records and unreferenced content; never recursively delete an unvalidated user-supplied path. Instance saves remain separate unless explicitly selected.

Assets/libraries may be shared in a launcher-managed content store for efficiency; each instance's mutable game directory remains isolated. A reference scan (not fragile manual refcounts) determines safe garbage collection.

## 10. Download architecture

`DownloadManager` receives immutable jobs; it does not understand Minecraft.

```text
DownloadPlan
  -> scheduler (priority + bounded semaphore)
  -> transfer (.part, streaming, timeout, optional Range)
  -> verifier (size/hash)
  -> atomic commit
  -> progress aggregator -> bounded AppEvent channel
```

- Separate connect, idle/read, and overall attempt timeouts.
- Retry transient network failures, 408, 429, and selected 5xx responses with exponential backoff, jitter, and `Retry-After`; do not retry deterministic 4xx/hash failures blindly.
- Cancellation is cooperative at request/chunk/retry boundaries and leaves only recognizable staging files.
- Resume only when the server confirms ranges and validators make continuity safe. Otherwise restart. Basic correctness ships before sophisticated resume.
- Redirect count is limited; final hosts must satisfy the request policy. HTTPS is required except loopback tests.
- Per-host and global concurrency are bounded. Defaults are conservative and configurable.
- Progress reports logical and network bytes separately so reused files do not create misleading speed.
- UI speed uses a smoothed window; ETA is omitted when unstable.

## 11. Java management

### Sources

1. Instance custom executable.
2. Launcher-managed runtime matching metadata component (Phase 2 download, model now).
3. Discovered system runtimes (`JAVA_HOME`, PATH, common registry/install locations via platform adapter).

Validation invokes the executable directly with bounded timeout and parses machine-readable properties where available, falling back to `java -version`. Capture vendor, runtime/home, major version, data model/architecture, and errors. Never infer suitability from filename alone.

Selection resolves the version metadata's Java component/major requirement first, then instance override. Older metadata receives a maintained compatibility policy with visible reasoning, not one universal Java assumption. The UI shows why a candidate is accepted/rejected. Managed runtime archives use the same safe download/extraction system and trusted manifests once implemented.

## 12. Instance architecture

Each instance directory is launcher-owned and named by `InstanceId`:

```text
instances/<uuid>/
├── instance.json
├── game/                 # saves, mods, options, screenshots
├── natives/              # regenerated, version-scoped subdirectories
├── logs/                 # per-launch captured logs
└── icon.*                # validated/copied local asset
```

Global immutable artifacts can be shared; mutable game state cannot. Duplicate copies metadata and optionally mutable game data through an explicit operation with size estimate and cancellation. Delete defaults to a recoverable trash/quarantine step when practical and always confirms whether game data/saves are included.

`LoaderSpec { kind, version, metadata }` is present conceptually, but MVP only allows `Vanilla`. Loader behavior is not a giant enum spread throughout the code: a future provider resolves loader metadata into the same normalized install and launch plans. The interface is introduced when Fabric is implemented, not prematurely.

## 13. Launch pipeline

1. Acquire an instance launch lock; prevent accidental duplicate launch unless explicitly supported.
2. Load instance/account/version data and validate schema/invariants.
3. Ensure authenticated Minecraft token/profile and entitlement; refresh if needed.
4. Validate installation (fast record check; targeted verification) and offer Repair on failure.
5. Resolve and validate Java against metadata requirement.
6. Evaluate library/argument rules for OS, architecture, and enabled features.
7. Prepare instance game/log/native directories; safely refresh natives when required.
8. Construct `LaunchPlan`: individual argument vector, classpath via platform separator, main class, working directory, and minimal environment changes.
9. Produce a **redacted** preview for diagnostics. Authentication placeholders are never retained in ordinary logs.
10. Spawn directly with Tokio/std process APIs—never via `cmd.exe`, PowerShell, or a shell string.
11. Read stdout/stderr concurrently using bounded line/chunk processing; write a per-session file and send throttled/bounded UI events.
12. Track states `Preparing -> Starting -> Running -> Exited/Failed/Killed`; permit Stop with graceful termination where feasible, then force kill after timeout.
13. Record exit code, duration, and diagnostic hints. Preserve raw game logs, but scan/redact launcher-injected secrets before persistence.

Argument placeholder substitution is typed and allowlisted. User JVM arguments are parsed as a list in the UI/storage; dangerous launcher-owned values (classpath, main class, auth placeholders, working directory semantics) cannot silently be overridden without an advanced warning/policy.

## 14. UI architecture and wireframes

### Visual direction

A calm dark/light adaptive desktop theme, strong typography hierarchy, 8px spacing grid, clear focus rings, restrained accent color, semantic status colors plus icons/text (never color alone), and progressive disclosure for technical detail. Minimum target window approximately 960×640; navigation can collapse at narrower widths. Respect OS scale settings and reduced-motion preferences where available.

### Shell

```text
┌────────────────────────────────────────────────────────────────────┐
│ Andromeda                         [task status] [account avatar ▾] │
├──────────────┬─────────────────────────────────────────────────────┤
│ Home         │                                                     │
│ Instances    │                 Current page                        │
│ Downloads    │                                                     │
│ Accounts     │                                                     │
│ Logs         │                                                     │
│ Settings     │                                                     │
├──────────────┴─────────────────────────────────────────────────────┤
│ ● Service/cache status        Version              Background task │
└────────────────────────────────────────────────────────────────────┘
```

### Home

```text
┌ Welcome, Alex ─────────────────────────────────────────────────────┐
│ Selected instance: [Vanilla 1.xx ▾]       [Edit]                  │
│ Version • Java • last played • installation health                 │
│                                                                    │
│                        [ PLAY ]                                    │
│              or [Install / Repair] with explanation               │
├ Recent news/status (only from a trusted configured source) ───────┤
│ Recent instances                         Recent launch result       │
└────────────────────────────────────────────────────────────────────┘
```

The primary action changes honestly between Play, Install, Repair, Sign in, or Select Java.

### Instances

```text
┌ Instances ─────────────────────────────────────── [+ New instance] ┐
│ Search [________]  Filter [All ▾]                                  │
│ ┌ cards/list ────────────────┐ ┌ Selected instance ─────────────┐ │
│ │ icon Name   version status │ │ Overview | Java | Game | Notes │ │
│ │ ...                       │ │ editable fields + validation    │ │
│ └────────────────────────────┘ │ [Save] [Duplicate] [Repair]     │ │
│                                │ [Delete…]                        │ │
└────────────────────────────────┴───────────────────────────────────┘
```

### Downloads/tasks

```text
┌ Downloads & Tasks ─────────────────────────────────────────────────┐
│ Installing 1.xx                                      [Cancel]      │
│ ███████████████░░ 73%  412 MiB / 563 MiB  18 MiB/s  ~8 sec       │
│ Verifying libraries • 84 / 112                                    │
│ [Details ▾: current files, retries, technical events]              │
├ Completed / Failed ────────────────────────────────────────────────┤
│ Task summary                            [Retry] [Open details]      │
└────────────────────────────────────────────────────────────────────┘
```

### Accounts

```text
┌ Accounts ──────────────────────────────────────────────────────────┐
│ [avatar] PlayerName    Minecraft owned    Session valid            │
│ Last checked …                         [Refresh] [Sign out]         │
│                                                                    │
│ [+ Sign in with Microsoft]                                      │
│ Privacy note: credentials are stored by Windows Credential Manager│
└────────────────────────────────────────────────────────────────────┘
```

### Settings

```text
┌ Settings ──────────────────────────────────────────────────────────┐
│ General      theme, language (future), close behavior              │
│ Java         discovered runtimes, default, validate/add custom     │
│ Downloads    concurrency, timeout, cache/storage locations         │
│ Minecraft    snapshots toggle, default RAM/resolution              │
│ Diagnostics  log level, open logs, export redacted bundle          │
│                                            [Restore defaults]      │
└────────────────────────────────────────────────────────────────────┘
```

### Logs/console

```text
┌ Logs ──────────────────────────────────────────────────────────────┐
│ Source [Game session ▾] Level [All ▾] Search [_______] [Follow ✓] │
│ 12:04:11 INFO  ...                                                 │
│ 12:04:12 WARN  ...                                                 │
│ [Copy selected] [Open file] [Export redacted diagnostics]          │
└────────────────────────────────────────────────────────────────────┘
```

Virtualize or cap displayed lines while retaining the file stream. Pause auto-follow when the user scrolls upward.

### Errors and confirmations

```text
┌ Could not verify the client file ────────────────────────┐
│ The downloaded file did not match the official checksum. │
│ No existing installation was replaced.                   │
│ [Try again] [Repair instance] [Cancel]                   │
│ Details ▾ expected…, received…, path…, task ID…          │
└───────────────────────────────────────────────────────────┘
```

Destructive confirmations name the instance and state whether saves are affected. Keyboard order, shortcuts, screen-reader labels where supported, and non-color statuses are acceptance criteria rather than polish deferred indefinitely.

## 15. Error-handling strategy

- Domain/service layers return typed errors with stable categories: authentication, entitlement, network, HTTP, integrity, metadata, filesystem, disk space, Java, configuration, launch, cancellation, and internal invariant.
- Each boundary adds operation context (`Downloading asset`, sanitized URL host, safe path, HTTP status, attempt/task ID) while preserving the source chain.
- UI maps errors to a short summary, impact, retryability, and one or more actions. Technical details are expandable.
- Cancellation is not logged or shown as an error.
- Panics represent bugs, not recoverable conditions. Startup installs a panic hook that writes a local redacted crash record; production paths do not use `unwrap`/`expect` except proven invariants documented nearby.
- Batch operations collect bounded representative failures plus counts rather than producing thousands of dialogs.

## 16. Security model

### Threat model

Potentially hostile inputs include remote metadata, redirects, archives, filenames, cached files modified by other software, imported instances, user-entered arguments/paths, game output, and local symlinks. Network attackers are constrained by TLS but official SHA-1 values primarily detect corruption and are not modern signatures. Malware already executing as the same user is largely out of scope, though credential vaults and least privilege reduce exposure.

### Controls

- HTTPS-only remote policy (except test/loopback), normal certificate validation, bounded redirects/timeouts/sizes, and an allowlist or explicit policy for official/trusted artifact hosts.
- Verify official size/hash whenever supplied before commit. Release/update artifacts eventually require modern digital signatures, not just hashes from the same channel.
- Canonical launcher-owned roots and safe relative path types; reject traversal, absolute/prefix paths, NULs, Windows device names, alternate data stream syntax, and unexpected separators.
- Archive extraction validates every entry and link type, enforces file-count/uncompressed-size/ratio limits, and never follows archive symlinks.
- Refuse or safely handle symlink/reparse-point traversal during privileged file operations and deletion.
- Spawn processes directly with argument arrays. Never interpolate a shell command. Validate custom Java executable as a file and display its resolved path.
- Run as the user; no administrator requirement. Keep installs in per-user application data by default.
- Store tokens only in OS credentials; redact tokens, authorization headers, player identifiers where appropriate, and launch arguments from logs/diagnostic exports.
- Write config atomically with restrictive practical permissions. Keep backup of last known good version and quarantine corrupt files.
- Bound metadata depth, collection counts, string/path lengths, response sizes, log memory, channels, retries, and concurrency to resist resource exhaustion.
- No remote analytics in MVP. Any future telemetry is explicit opt-in and documented.

## 17. Storage/configuration design

Use OS-known folders resolved by a platform path service. Conceptually:

```text
Config/Andromeda/
  settings.json
  accounts.json                  # summaries only
Data/Andromeda/
  instances/<id>/...
  minecraft/{versions,libraries,assets}/...
  runtimes/...
  logs/{launcher,game}/...
Cache/Andromeda/
  metadata/...
  downloads/*.part
  diagnostics/...
Credential Manager:
  service=Andromeda, account=<AccountId> -> token secret
```

Exact Windows paths come from known-folder APIs through the chosen directories crate and are shown in Settings.

- JSON is appropriate for human-inspectable settings, instances, and metadata caches.
- Every owned document has a schema version and is deserialized into a persisted DTO, migrated, validated, then converted to domain types.
- Save using sibling temp file, flush/sync as warranted, atomic rename/replace, then maintain a bounded previous-good backup.
- On corruption: preserve/quarantine the original, try last-known-good, otherwise use safe defaults and show a persistent recovery notice. Never silently discard instance metadata.
- Cache is disposable and can be rebuilt. Secrets and settings are separate. Logs are rotated by size/time with retention settings.
- Custom/external directories require explicit ownership semantics; launcher cleanup never assumes it may delete their parent.

## 18. Logging strategy

- `tracing` spans correlate startup, auth flow, task, instance, download, install, and launch session IDs.
- Development default: readable console + verbose file. Release default: INFO file with bounded rotation; game output has separate session files.
- Levels: TRACE/DEBUG diagnostic internals, INFO lifecycle, WARN recoverable degradation, ERROR failed operation. Fatal startup failure is represented explicitly and exits only when no degraded mode is safe.
- Redaction occurs before formatting/sinks and is tested. Secret wrapper types do not implement revealing `Debug`/`Display`.
- UI receives sanitized structured summaries, not the entire tracing stream.
- A diagnostics export is opt-in, previews included files, redacts secrets and sensitive paths where practical, and never includes credential-vault content.

## 19. Testing strategy

### Unit tests

- Rule evaluation matrices (OS/architecture/features).
- Argument placeholder expansion and individual argument construction.
- Java version parsing and requirement matching.
- Hash/size validation, retry classification, progress aggregation.
- Instance/config validation and migrations.
- Safe relative paths, Windows reserved names, traversal, symlink/archive policies.

### Integration/contract tests

- Pinned sanitized official metadata fixtures for representative legacy and modern versions, asset indexes, native classifiers, logging config, and Java requirements.
- Install-plan golden tests without network or actual downloads.
- Mock HTTP tests: redirects, range support/ignore, truncation, timeout, 429 retry, checksum mismatch, cancellation, and atomic commit.
- Temp-directory repository recovery, duplicate/delete boundaries, and corrupt config behavior.
- Launch-plan snapshots with all secrets replaced by explicit redaction markers.
- Fake process runner for stdout/stderr, exit code, timeout, and kill behavior.
- Secret-store fake verifying token lifecycle and log non-disclosure.

### End-to-end/manual tests

- A gated Windows smoke suite using a test app registration/account supplied only in CI secrets; never required for ordinary contributors.
- Install/repair/launch checks for a supported version matrix and Java majors.
- DPI/scaling, keyboard navigation, offline startup, low disk, read-only directory, proxy/firewall, and interrupted download scenarios.

CI will run formatting, Clippy with project-agreed warnings, tests, dependency policy (`cargo-deny`), and vulnerability audit. Live service tests are scheduled/manual to avoid flaky pull requests.

## 20. Build and release strategy

- Stable Rust pinned by `rust-toolchain.toml`; `Cargo.lock` committed.
- Debug builds include developer console/file diagnostics; release builds optimize appropriately and retain useful symbols in separate release artifacts if feasible.
- CI starts with Windows build/test; Linux compilation is added early to detect accidental platform coupling, even before Linux UX is supported.
- Windows packaging candidate: MSIX if signing/distribution requirements fit; otherwise a signed WiX-based MSI. Decide after a packaging spike. Avoid admin rights and machine-wide writes.
- Semantic product versions with release notes and schema migration compatibility policy.
- Release artifacts, installer, checksums, and eventually update metadata are code-signed in protected CI using short-lived/secured signing access.
- Reproducibility inputs are locked; license and dependency reports accompany releases.
- Automatic update is deferred until core stabilization. Its future design requires signed manifests, staged download, atomic handoff, rollback, and channel support.

## 21. Development roadmap

1. **Milestone 1 — Foundations and architecture validation:** project skeleton, domain primitives, config/path foundations, tracing, Iced shell spike, CI/tests; no Minecraft install yet.
2. **Milestone 2 — Metadata and rules:** official manifests, cache, typed metadata, rule evaluator, available-version UI with fixtures/mocks.
3. **Milestone 3 — Download engine:** generic plans, staging, hashing, retries, cancellation, progress, mock-server integration tests.
4. **Milestone 4 — Instances and Java:** CRUD/migrations, isolated paths, system/custom Java discovery and validation, UI editors.
5. **Milestone 5 — Vanilla installation/repair:** assets, libraries, natives, client/log config, disk preflight, install/repair UI.
6. **Milestone 6 — Authentication:** project-owned app registration, PKCE/browser flow, Xbox/Minecraft exchanges, entitlement/profile, keyring, account UI.
7. **Milestone 7 — Launch engine:** normalized arguments, classpath/native prep, process/log lifecycle, diagnostics, Home Play flow.
8. **Milestone 8 — MVP hardening:** offline/recovery behavior, accessibility/usability, security tests, representative version matrix, performance, docs.
9. **Milestone 9 — Windows release:** icon/metadata, signed installer pipeline, release checklist, clean-machine testing.
10. **Post-MVP:** managed runtimes, multiple accounts, import/export, then one loader provider at a time.

Authentication is scheduled after deterministic infrastructure to avoid making early development depend on credentials. A small endpoint/app-registration proof may run earlier as a time-boxed risk spike.

## 22. Risks and difficult areas

| Risk | Impact | Mitigation |
|---|---|---|
| Microsoft app registration and evolving auth/Xbox chain | Blocks sign-in | Verify current official requirements early; project-owned client ID; isolated provider and contract tests; risk spike before Milestone 3 ends. |
| Minecraft metadata variants across eras | Install/launch failures | Fixture matrix, normalized model, shared rule engine, declare a tested support range for MVP rather than claiming every historical version. |
| Java compatibility by Minecraft version | Confusing launch failures | Metadata-first requirement, validated runtime details, compatibility table only for missing legacy metadata. |
| Windows filesystem locks/path rules/antivirus | Failed atomic commits or deletion | Same-volume staging, retry sharing violations conservatively, reserved-name/path tests, useful diagnostics. |
| Native extraction and untrusted archives | Security compromise | Central safe extractor, no links, size/count limits, clean destination, adversarial tests. |
| Token leakage through launch/log/error paths | Account compromise | Secret types, redaction at construction and sinks, tests with canary tokens, diagnostics review. |
| UI framework limitations/accessibility | Rework | Milestone 1 objective spike and documented fallback to Tauri before core UI grows. |
| Progress event volume and huge logs | UI stalls/memory growth | Bounded channels, aggregation/throttling, virtual/capped views, stream to files. |
| Download resume correctness | Corruption | Ship restart-safe staging first; resume only with validated range/validator semantics. |
| Shared-store garbage collection | Data loss | Reference scan, quarantine, launcher-owned roots only; defer automatic GC until proven. |
| Scope expansion into mod management | Delays reliable MVP | Vanilla acceptance gate; no loader abstraction implementation until one real provider is scheduled. |
| Upstream terms/API changes | Maintenance/legal risk | Official services/resources only, documented endpoints, no bypass, review terms before release. |

## 23. Detailed Milestone 1

### Objective

Establish a production-quality foundation and validate the riskiest early architectural choice (Iced) without implementing authentication, downloading, or Minecraft launching.

### Deliverables

1. Cargo package with `lib.rs` and minimal `main.rs`, stable toolchain, formatting/lint policy, dependency policy, and Windows CI.
2. Initial modules only where exercised: `bootstrap`, `app`, `domain`, `config`, `storage`, `platform`, `telemetry`, and `ui`.
3. Typed IDs, validated launcher paths, `AppError` skeleton, task lifecycle/progress primitives, and non-secret settings model.
4. Platform-resolved application directories with overrides only for tests/development.
5. Versioned settings repository with atomic save, corrupt-file quarantine/recovery, and tests.
6. Structured rotating logs with a tested redaction wrapper and startup span.
7. Iced shell implementing navigation and representative static/fixture states for Home, Instances, Downloads, Accounts, Settings, Logs, an error dialog, and a progress task.
8. A fake background task proving cancellation, bounded/throttled progress, and responsive UI state transitions.
9. Theme tokens, focus/keyboard behavior baseline, scale/resizing check, and no hard-coded page colors.
10. README setup/build/test instructions, accepted ADRs for GUI and single-package structure, and updated blueprint status.

### Files expected

- Root: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `deny.toml`, `.gitignore`, CI workflow, expanded `README.md`.
- Source subset described above; no empty speculative module forest.
- Tests for settings recovery, safe paths, redaction, reducer/task transitions.
- `docs/decisions/0001-gui-iced.md` and `0002-single-package.md` after owner approval.

### Acceptance criteria

- `cargo fmt --check`, `cargo clippy --all-targets --all-features`, and `cargo test --all-targets` pass on the pinned stable toolchain.
- A Windows build opens a responsive shell; all six pages are keyboard reachable and usable at target minimum size and 125%/150% scaling.
- A fake task can start, update at a bounded rate, cancel, and finish without blocking rendering.
- Settings survive restart; malformed settings are preserved/quarantined and produce a visible recovery notice rather than a crash.
- Logs are written to the correct platform directory and a canary secret never appears in console/file/UI output.
- Core tests run without creating a GUI or requiring network/account credentials.
- No production `unwrap`/`expect` except documented startup invariants; no shell process execution; no tokens or credentials in fixtures.

### Known Milestone 1 limitations

It will not contact Minecraft services, authenticate, discover Java, install versions, or launch the game. Page content is representative fixture data explicitly marked as demonstration state. The purpose is to validate architecture, interaction patterns, persistence, and build health.

### Implementation sequence once approved

1. Confirm decisions listed below.
2. Create toolchain/package/CI and a minimal headless library test.
3. Implement domain primitives and app paths.
4. Implement settings persistence/recovery and tracing/redaction.
5. Add app reducer/task demonstration.
6. Build and evaluate the Iced shell against acceptance criteria.
7. Run checks, update documentation, and report limitations before beginning Milestone 2.

## 24. Decisions requested from the project owner

All six decisions below were approved by the project owner on 2026-09-19:

1. GUI: **approve Iced with a Milestone 1 exit criterion**, or choose Tauri/egui now.
2. Version catalog: **show all entries from Mojang’s live manifest and refresh automatically**, so newly published versions appear without launcher updates. Compatibility is metadata-driven; versions that cannot be safely resolved must show a precise unsupported-metadata error rather than disappear or launch incorrectly.
3. Accounts: **one active account UX in MVP with multi-account-capable storage**, full switching in Phase 2.
4. Instance deletion: **quarantine/trash when practical plus explicit permanent cleanup**, rather than immediate recursive deletion.
5. Java: **system/custom Java in MVP; managed downloads in Phase 2**.
6. Packaging: **defer MSIX versus MSI choice to a small release spike**, while keeping per-user/no-admin constraints now.

## 25. Architecture decision summary

The proposal deliberately chooses a modular monolith, pure Rust Iced UI, Tokio/Reqwest backend, JSON documents plus platform credentials, typed plans for installation/launch, and narrow traits at external boundaries. This solves present testability and safety needs without committing early to micro-crates, a database, plugins, managed Java distribution, or multiple loader frameworks.

The owner approved these decisions. Milestone 1 implementation may proceed within its stated deliverables.
