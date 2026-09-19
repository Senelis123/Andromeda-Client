# ADR 0001: Use Iced for the desktop UI

- Status: Accepted
- Date: 2026-09-19

## Decision

Use Iced 0.14 as the pure-Rust desktop UI toolkit. Milestone 1 is an exit-gated spike: keyboard use, scaling, responsive task updates, minimum-size layout, and Windows build viability must be validated before the UI grows.

## Consequences

The UI follows typed messages and immutable views and keeps service logic outside widgets. Styling and accessibility require deliberate testing. If the exit criteria fail, Tauri is the documented fallback before Minecraft service work is coupled to the UI.
