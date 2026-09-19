# ADR 0002: Begin as a modular single package

- Status: Accepted
- Date: 2026-09-19

## Decision

Use one Cargo package with a testable library and thin binary. Enforce boundaries through modules and dependency direction rather than multiple crates.

## Consequences

This minimizes workspace and release complexity while preserving headless tests. A core crate may be extracted later if compile times, reuse, or independently enforceable boundaries justify it.
