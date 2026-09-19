# Milestone 2 — Minecraft metadata and rules

## Implemented

- Automatic retrieval of Mojang's official `version_manifest_v2.json` at startup.
- Conditional HTTP refresh using the cached ETag.
- Last-known-good offline fallback with a visible warning.
- An 8 MiB response limit, HTTPS enforcement, bounded redirects/timeouts, safe version-ID validation, and manifest count limits.
- Typed global and per-version metadata for modern/legacy arguments, downloads, libraries, natives, assets, logging, Java requirements, and inheritance.
- Mojang OS/architecture/feature rule evaluation using ordered last-match semantics.
- Live version summary on Home and a catalog list on Instances.
- Release filtering by default and a persisted snapshot visibility setting.
- Fixture-based serialization and rule tests that do not require live network access.

## Data flow

At application boot, an Iced task calls `VersionCatalogService`. A successful network response is validated and atomically cached. HTTP 304 reuses the cache. Network or HTTP failure uses a valid cache when available and reports degraded/offline state. The resulting typed manifest is reduced into `CatalogState` and rendered by the UI.

## Known limitations

- Per-version documents are modeled but fetched during installation planning in a later milestone.
- No version can be installed yet.
- Mojang's manifest does not publish a stable ETag on every edge response; a normal validated refresh is used in that case.
- The current list intentionally displays the first 40 matching entries pending a virtualized/searchable catalog component.
