// @synapse/plugin — the public plugin API contract.
//
// This is the *stable* surface third-party plugins compile against. Internal
// refactors must not break it. The SDK ships in two parts:
//   1. Types only (this file) — define the manifest, capabilities, and surfaces.
//   2. Extension registry (`ExtensionRegistry`) — the host-side API; plugins
//      receive a scoped view of it via `PluginContext`.
//
// Two plugin surfaces exist (see ARCHITECTURE §7):
//   • UI plugins run in a sandboxed worker/iframe and use `PluginContext`.
//   • Core hooks run in a sandboxed JS/Wasm runtime and react to DomainEvents.
// Both are capability-scoped: a plugin only receives what its manifest declares.
//
// AI extension points are defined here (§ai) but the execution runtime is
// post-MVP. A plugin may declare an AI capability and the host will call it when
// that extension point is invoked.

// ── Capabilities ─────────────────────────────────────────────────────────────

/** Capabilities a plugin may request. The host grants only what is declared. */
export type Capability =
  | "read:notes"
  | "write:cards"
  | "read:decks"
  | "ui:panel"
  | "ui:command"
  | "ui:review-button"
  | "net:fetch"
  | "events:listen"
  | "ai:card-hint"
  | "ai:note-fill-back"
  | "ai:deck-summary";

// ── Manifest ─────────────────────────────────────────────────────────────────

/** Plugin manifest (`plugin.json`). */
export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  /** Module entry point relative to the plugin root. */
  entry: string;
  description?: string;
  author?: string;
  /** Minimum Synapse plugin-API version this plugin supports. */
  apiVersion: string;
  permissions: Capability[];
}

// ── Domain events ─────────────────────────────────────────────────────────────

/** Events a plugin can subscribe to. Mirror of `synapse_core::DomainEvent`. */
export type PluginEvent =
  | { type: "card-answered"; cardId: number }
  | { type: "note-added"; noteId: number }
  | { type: "note-updated"; noteId: number }
  | { type: "note-removed"; noteId: number }
  | { type: "deck-changed"; deckId: number }
  | { type: "collection-opened" }
  | { type: "collection-closed" };

// ── UI extension points ───────────────────────────────────────────────────────

export interface CommandDefinition {
  id: string;
  title: string;
  /** Optional default keybinding, e.g. "mod+shift+k". */
  keybinding?: string;
  run: () => void | Promise<void>;
}

export interface SidebarPanel {
  id: string;
  title: string;
  icon?: string;
  /** Mount point; the host renders the returned element in the sidebar. */
  mount: (container: HTMLElement) => void | (() => void);
}

export interface ReviewButton {
  id: string;
  label: string;
  onClick: (cardId: number) => void;
}

/** A custom renderer that can transform card HTML before display. */
export interface CardRenderer {
  id: string;
  /** Return transformed HTML (or the input unchanged). */
  render: (html: string, ctx: { cardId: number; side: "front" | "back" }) => string;
}

// ── AI extension points ───────────────────────────────────────────────────────

/** Input to an AI extension-point handler. */
export interface AITextRequest {
  prompt: string;
  /** Additional context the host provides (e.g. field values, deck name). */
  context?: Record<string, string>;
  maxTokens?: number;
}

/** Output from an AI extension-point handler. */
export interface AITextResponse {
  text: string;
}

/**
 * A plugin that provides AI capabilities registers handlers for one or more
 * extension-point ids (see `synapse_core::ai::extension_point`).
 * The host calls the first registered handler that covers the requested id.
 */
export interface AIHandler {
  /** Extension-point id, e.g. `"ai.card_hint"`. */
  extensionPoint: string;
  handle: (request: AITextRequest) => AITextResponse | Promise<AITextResponse>;
}

// ── Core hook surface ─────────────────────────────────────────────────────────

/**
 * Plugins that declare only logic (no UI) implement `CoreHookModule` instead of
 * `PluginModule`. The host instantiates them in the core-hook runtime (post-MVP).
 */
export interface CoreHookModule {
  /** Called once on activation. Return cleanup to run on deactivate. */
  activate(ctx: CoreHookContext): void | (() => void) | Promise<void | (() => void)>;
}

/** Context provided to a core-hook plugin. */
export interface CoreHookContext {
  readonly manifest: PluginManifest;
  onEvent(handler: (event: PluginEvent) => void): Disposable;
  registerAIHandler(handler: AIHandler): Disposable;
}

// ── Plugin host context ───────────────────────────────────────────────────────

/** Handle passed to a UI plugin's `activate(ctx)` entry point. */
export interface PluginContext {
  readonly manifest: PluginManifest;
  registerCommand(command: CommandDefinition): Disposable;
  addSidebarPanel(panel: SidebarPanel): Disposable;
  addReviewButton(button: ReviewButton): Disposable;
  registerCardRenderer(renderer: CardRenderer): Disposable;
  registerAIHandler(handler: AIHandler): Disposable;
  onEvent(handler: (event: PluginEvent) => void): Disposable;
}

/** Returned by registration calls so plugins can clean up on deactivate. */
export interface Disposable {
  dispose(): void;
}

/** Shape a UI plugin's entry module must export. */
export interface PluginModule {
  activate(ctx: PluginContext): void | Promise<void>;
  deactivate?(): void | Promise<void>;
}

// ── Extension registry (host-side) ────────────────────────────────────────────

/**
 * The host-side registry. Plugins register into it; the application queries it.
 * The actual implementation lives in the shell and is NOT part of the plugin SDK
 * stable surface — only the types below are stable.
 */
export interface ExtensionRegistry {
  /** All commands currently registered (navigation + plugin contributions). */
  readonly commands: ReadonlyArray<CommandDefinition & { pluginId: string }>;
  /** All AI handlers currently registered, keyed by extension-point id. */
  readonly aiHandlers: ReadonlyMap<string, AIHandler>;
  /** All sidebar panels. */
  readonly panels: ReadonlyArray<SidebarPanel & { pluginId: string }>;
  /** All review buttons. */
  readonly reviewButtons: ReadonlyArray<ReviewButton & { pluginId: string }>;
  /** All card renderers, in registration order (applied as a pipeline). */
  readonly cardRenderers: ReadonlyArray<CardRenderer>;

  /** Invoke the AI handler for `extensionPoint`, if one is registered. */
  invokeAI(extensionPoint: string, request: AITextRequest): Promise<AITextResponse | null>;
}

// ── Version ───────────────────────────────────────────────────────────────────

/** Current plugin API version. Bumped on breaking changes to this contract. */
export const PLUGIN_API_VERSION = "0.1.0";
