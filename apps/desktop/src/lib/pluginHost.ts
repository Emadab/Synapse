/**
 * Plugin host: creates one sandboxed Worker per enabled plugin.
 *
 * Each plugin runs its entry-point JS inside a Web Worker. The host injects
 * a bridge preamble before the plugin code that exposes a capability-gated
 * `SynapsePlugin` global. Communication is via `postMessage`:
 *
 *   Plugin → Host  (registrations + actions)
 *     { type: "register:command", id, title }
 *     { type: "register:panel",   id, title, content }
 *     { type: "show:toast",       message }
 *
 *   Host → Plugin  (invocations + events)
 *     { type: "invoke:command", id }
 *     { type: "event",          event }
 */

export interface PluginCommand {
  pluginId: string;
  id: string;
  title: string;
}

export interface PluginPanel {
  pluginId: string;
  id: string;
  title: string;
  content: string;
}

export type PluginToast = { pluginId: string; message: string };

export interface PluginHostHandlers {
  onCommand: (cmd: PluginCommand) => void;
  onPanel: (panel: PluginPanel) => void;
  onToast: (toast: PluginToast) => void;
}

/** Manages the lifecycle of one plugin Worker. */
export class PluginWorker {
  private worker: Worker;

  constructor(
    public readonly pluginId: string,
    permissions: string[],
    entryCode: string,
    private readonly handlers: PluginHostHandlers,
  ) {
    const code = buildBridge(permissions) + "\n;\n" + entryCode;
    const blob = new Blob([code], { type: "application/javascript" });
    const url = URL.createObjectURL(blob);
    this.worker = new Worker(url);
    URL.revokeObjectURL(url);

    this.worker.onmessage = (e: MessageEvent) => this.onMessage(e.data);
    this.worker.onerror = (e) => {
      console.error(`[plugin:${pluginId}] worker error:`, e.message);
    };
  }

  private onMessage(msg: Record<string, unknown>) {
    switch (msg.type) {
      case "register:command":
        this.handlers.onCommand({
          pluginId: this.pluginId,
          id: String(msg.id),
          title: String(msg.title),
        });
        break;
      case "register:panel":
        this.handlers.onPanel({
          pluginId: this.pluginId,
          id: String(msg.id),
          title: String(msg.title),
          content: String(msg.content ?? ""),
        });
        break;
      case "show:toast":
        this.handlers.onToast({ pluginId: this.pluginId, message: String(msg.message) });
        break;
    }
  }

  invokeCommand(id: string) {
    this.worker.postMessage({ type: "invoke:command", id });
  }

  sendEvent(event: unknown) {
    this.worker.postMessage({ type: "event", event });
  }

  destroy() {
    this.worker.terminate();
  }
}

/** Host: manages all active plugin Workers. */
export class PluginHost {
  private workers = new Map<string, PluginWorker>();
  readonly commands: PluginCommand[] = [];
  readonly panels: PluginPanel[] = [];
  private toastListeners: Array<(t: PluginToast) => void> = [];

  private makeHandlers(): PluginHostHandlers {
    return {
      onCommand: (cmd) => {
        if (!this.commands.find((c) => c.id === cmd.id)) {
          this.commands.push(cmd);
        }
      },
      onPanel: (panel) => {
        if (!this.panels.find((p) => p.id === panel.id)) {
          this.panels.push(panel);
        }
      },
      onToast: (toast) => {
        this.toastListeners.forEach((fn) => fn(toast));
      },
    };
  }

  onToast(fn: (t: PluginToast) => void) {
    this.toastListeners.push(fn);
    return () => {
      this.toastListeners = this.toastListeners.filter((f) => f !== fn);
    };
  }

  load(pluginId: string, permissions: string[], entryCode: string) {
    this.unload(pluginId);
    const worker = new PluginWorker(pluginId, permissions, entryCode, this.makeHandlers());
    this.workers.set(pluginId, worker);
  }

  unload(pluginId: string) {
    const existing = this.workers.get(pluginId);
    if (existing) {
      existing.destroy();
      this.workers.delete(pluginId);
      // Remove contributed commands/panels from this plugin
      const idx = this.commands.findIndex((c) => c.pluginId === pluginId);
      if (idx !== -1) this.commands.splice(idx, 1);
      const pidx = this.panels.findIndex((p) => p.pluginId === pluginId);
      if (pidx !== -1) this.panels.splice(pidx, 1);
    }
  }

  invokeCommand(id: string) {
    for (const worker of this.workers.values()) {
      worker.invokeCommand(id);
    }
  }

  broadcastEvent(event: unknown) {
    for (const worker of this.workers.values()) {
      worker.sendEvent(event);
    }
  }

  destroyAll() {
    for (const worker of this.workers.values()) {
      worker.destroy();
    }
    this.workers.clear();
    this.commands.length = 0;
    this.panels.length = 0;
  }
}

/** Singleton host shared across the app. */
export const pluginHost = new PluginHost();

// ── Bridge preamble ────────────────────────────────────────────────────────────

function buildBridge(permissions: string[]): string {
  return `
(function() {
  var _perms = ${JSON.stringify(permissions)};
  function _check(cap) {
    if (_perms.indexOf(cap) === -1) throw new Error("Plugin permission denied: " + cap);
  }
  var _commandHandlers = {};
  var _eventHandler = null;

  self.SynapsePlugin = {
    registerCommand: function(id, title) {
      _check("ui:command");
      self.postMessage({ type: "register:command", id: id, title: title });
    },
    addSidebarPanel: function(id, title, content) {
      _check("ui:panel");
      self.postMessage({ type: "register:panel", id: id, title: title, content: content });
    },
    showToast: function(message) {
      self.postMessage({ type: "show:toast", message: message });
    },
    onCommand: function(id, handler) {
      _commandHandlers[id] = handler;
    },
    onEvent: function(handler) {
      _check("events:listen");
      _eventHandler = handler;
    },
  };

  self.addEventListener("message", function(e) {
    var data = e.data;
    if (data.type === "invoke:command" && _commandHandlers[data.id]) {
      try { _commandHandlers[data.id](); } catch(err) { console.error(err); }
    }
    if (data.type === "event" && _eventHandler) {
      try { _eventHandler(data.event); } catch(err) { console.error(err); }
    }
  });
})();
`;
}
