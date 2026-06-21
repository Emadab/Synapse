# ADR-003: Plugin sandbox via Web Workers (not Wasm runtime)

**Status:** Accepted  
**Date:** 2025-06-01

## Context

Synapse needs a plugin runtime that:
- Sandboxes untrusted JS code (no DOM access, no arbitrary fs/net)
- Enforces capability declarations from the plugin manifest
- Works without adding new native Rust dependencies (network blocked in CI; `wasmtime` and `rquickjs` can't be fetched)
- Can host the plugin SDK API surface (`registerCommand`, `addSidebarPanel`, `showToast`, `onEvent`)

## Decision

Run each plugin in a **browser Web Worker** created from a `Blob` URL. Before executing plugin code, inject a bridge IIFE that:

1. Reads the plugin's declared capabilities from the manifest.
2. Exposes `self.SynapsePlugin = { registerCommand, addSidebarPanel, showToast, onCommand, onEvent }`.
3. Guards every host-side capability behind `_check(cap)` which throws `PermissionDenied` if the capability is not in the manifest.
4. Leaves network APIs (`fetch`, `XMLHttpRequest`) unavailable — Workers do have `fetch` but the Tauri CSP blocks external origins.

`PluginHost` in `src/lib/pluginHost.ts` manages the Worker lifecycle, routes `postMessage` calls to host handlers, and exposes the plugin's registered commands to the UI.

## Consequences

- No new Rust dependency; sandbox is provided by the browser engine Tauri embeds.
- Worker thread isolation is real: plugin code cannot access `document`, `window`, or the main thread's memory.
- Capability enforcement is JS-side (in the bridge preamble). A malicious plugin could bypass it by using raw `postMessage` — acceptable for the plugin threat model (user installs plugins manually; no marketplace yet).
- Audio/DOM rendering extensions must go through the bridge API; direct DOM manipulation from plugins is not possible.
- If a Wasm runtime becomes available (v1.1+), the bridge API surface is stable and the sandbox can be swapped without changing plugin manifests.
