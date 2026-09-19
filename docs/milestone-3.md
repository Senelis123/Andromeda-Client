# Milestone 3 — Reusable download engine

## Implemented

- Minecraft-independent `DownloadPlan`, `DownloadJob`, and `DownloadManager` APIs.
- Globally bounded asynchronous concurrency with configurable connect, idle, and attempt timeouts.
- Cooperative cancellation while queued, requesting, streaming, or waiting to retry.
- Bounded-channel lifecycle and aggregate progress events, including separate logical and network byte counts.
- Retry of transport errors, timeouts, HTTP 408/429, and server errors with bounded exponential backoff.
- Same-directory recognizable partial and resume metadata files.
- Safe HTTP resume only when a saved ETag/Last-Modified validator is available and the server confirms the exact range with `206` and `Content-Range`; ignored ranges restart from byte zero.
- Streaming transfers and streaming SHA-1/SHA-256 verification, with optional exact-size enforcement.
- Atomic same-volume rename only after verification; failed or cancelled transfers never publish the destination.
- Cross-platform relative-path validation, duplicate destination detection, HTTPS enforcement (with a loopback-only test exception), and symlink/device-name defenses.
- Offline mock-server coverage for verified commits, safe resume, retries, and cancellation, plus unit tests for paths, checksums, and retry classification.

## Public flow

1. Create a manager with a launcher-owned destination root and `DownloadConfig`.
2. Build an immutable plan whose destinations are relative to that root.
3. Pass a `CancellationToken` and bounded Tokio event sender to `execute`.
4. Consume `DownloadEvent`s for UI updates and use `DownloadSummary` as the authoritative final count.

The manager deliberately knows nothing about Minecraft. A later installation planner will map official metadata to jobs, deduplicate artifacts, and invoke this engine.

## Resume and integrity policy

Partial data uses `.<filename>.andromeda.part` next to the final file, guaranteeing that commit does not cross filesystems. Validator metadata uses `.<filename>.andromeda.resume.json`. A partial without matching URL and validator metadata is truncated rather than appended. Hash mismatches and deterministic client errors are not retried blindly.

SHA-1 is supported because Mojang metadata publishes it for many artifacts; SHA-256 is available for modern/local integrity records. Hashes detect corruption but do not authenticate an untrusted source.
