// @synapse/plugin — the public plugin API contract.
//
// This is the *stable* surface third-party plugins compile against. Internal
// refactors must not break it. The MVP ships these types and the extension-point
// registry only; the sandboxed runtime that executes plugins is post-MVP.
//
// Two plugin surfaces exist (see ARCHITECTURE §7):
//   • UI plugins run in a sandboxed worker/iframe and use `PluginContext`.
//   • Core hooks run in a sandboxed JS/Wasm runtime and react to DomainEvents.
// Both are capability-scoped: a plugin only receives what its manifest declares.

/** Capabilities a plugin may request. The host grants only what is declared. */
export type Capability =
  | "read:notes"
  | "write:cards"
  | "read:decks"
  | "ui:panel"
  | "ui:command"
  | "ui:review-button"
  | "net:fetch"
  | "events:listen";

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

/** Events a plugin can subscribe to. Mirror of `synapse_core::DomainEvent`. */
export type PluginEvent =
  | { type: "card-answered"; cardId: number }
  | { type: "note-added"; noteId: number }
  | { type: "note-updated"; noteId: number }
  | { type: "note-removed"; noteId: number }
  | { type: "deck-changed"; deckId: number }
  | { type: "collection-opened" }
  | { type: "collection-closed" };

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

/** Handle passed to a UI plugin's `activate(ctx)` entry point. */
export interface PluginContext {
  readonly manifest: PluginManifest;
  registerCommand(command: CommandDefinition): Disposable;
  addSidebarPanel(panel: SidebarPanel): Disposable;
  addReviewButton(button: ReviewButton): Disposable;
  registerCardRenderer(renderer: CardRenderer): Disposable;
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

/** Current plugin API version. Bumped on breaking changes to this contract. */
export const PLUGIN_API_VERSION = "0.1.0";
