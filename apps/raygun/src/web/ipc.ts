// Raygun on the web (docs/WEB.md): stands in for Tauri's IPC. The generated bindings call
// `window.__TAURI_INTERNALS__.invoke` for commands and the event plugin's `listen`/`emit`; here
// both go to the web core in the page instead of to Rust.

/** Answers one command by its Rust name. A thrown string is the command's error, as Rust's `Err`. */
export type InvokeHandler = (command: string, args: Record<string, unknown>) => Promise<unknown>;

/** The event side of the stand-in: what the web core uses to tell the page something changed. */
export type WebIpc = {
  /** Delivers an event to the page's listeners, as Rust's `emit` does. */
  emit: (event: string, payload: unknown) => void;
  /** Takes the stand-in away again (tests). */
  uninstall: () => void;
};

type Callback = (data: unknown) => void;

type TauriInternals = {
  invoke: (command: string, args?: Record<string, unknown>, options?: unknown) => Promise<unknown>;
  transformCallback: (callback?: Callback, once?: boolean) => number;
  unregisterCallback: (id: number) => void;
  runCallback: (id: number, data: unknown) => void;
  callbacks: Map<number, Callback>;
  convertFileSrc: (path: string, protocol?: string) => string;
};

type WebWindow = Window & {
  __TAURI_INTERNALS__?: Partial<TauriInternals>;
  __TAURI_EVENT_PLUGIN_INTERNALS__?: { unregisterListener?: (event: string, id: number) => void };
};

/** What a failed command rejects with: Rust's errors are strings. */
const asError = (error: unknown) => {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return String(error);
};

/** Installs the stand-in on `window`; every command goes to `handle`. */
export const installWebIpc = (handle: InvokeHandler): WebIpc => {
  const target = window as WebWindow;
  const callbacks = new Map<number, Callback>();
  // event name → listener id → callback id
  const listeners = new Map<string, Map<number, number>>();
  let nextListener = 1;

  const transformCallback = (callback?: Callback, once = false) => {
    let id = 0;
    do {
      id = crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;
    } while (id === 0 || callbacks.has(id));
    callbacks.set(id, (data) => {
      if (once) callbacks.delete(id);
      callback?.(data);
    });
    return id;
  };
  const unregisterCallback = (id: number) => {
    callbacks.delete(id);
  };
  const runCallback = (id: number, data: unknown) => callbacks.get(id)?.(data);

  const emit = (event: string, payload: unknown) => {
    const forEvent = listeners.get(event);
    if (!forEvent) return;
    for (const [listener, callback] of [...forEvent]) {
      try {
        runCallback(callback, { event, id: listener, payload });
      } catch (error) {
        console.error(`[raygun web] a listener of ${event} failed`, error);
      }
    }
  };

  const unlisten = (event: string, listener: number) => {
    const forEvent = listeners.get(event);
    const callback = forEvent?.get(listener);
    forEvent?.delete(listener);
    if (callback !== undefined) unregisterCallback(callback);
  };

  const eventPlugin = (command: string, args: Record<string, unknown>) => {
    const event = String(args.event ?? '');
    switch (command) {
      case 'plugin:event|listen': {
        const listener = nextListener++;
        const forEvent = listeners.get(event) ?? new Map<number, number>();
        forEvent.set(listener, Number(args.handler));
        listeners.set(event, forEvent);
        return listener;
      }
      case 'plugin:event|unlisten':
        unlisten(event, Number(args.eventId));
        return null;
      case 'plugin:event|emit':
      case 'plugin:event|emit_to':
        // Deliver after the caller's turn, as an event from Rust arrives.
        queueMicrotask(() => emit(event, args.payload));
        return null;
      default:
        return null;
    }
  };

  const invoke = async (command: string, args: Record<string, unknown> = {}) => {
    if (command.startsWith('plugin:event|')) return eventPlugin(command, args);
    // Other plugins (windows, webviews) don't exist in a browser tab.
    if (command.startsWith('plugin:')) throw `${command} isn't available on the web`;
    try {
      return await handle(command, args);
    } catch (error) {
      throw asError(error);
    }
  };

  const previous = target.__TAURI_INTERNALS__;
  const previousEvents = target.__TAURI_EVENT_PLUGIN_INTERNALS__;
  target.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback,
    unregisterCallback,
    runCallback,
    callbacks,
    convertFileSrc: (path: string) => path,
  };
  target.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event: string, id: number) => unlisten(event, id),
  };

  return {
    emit,
    uninstall: () => {
      target.__TAURI_INTERNALS__ = previous;
      target.__TAURI_EVENT_PLUGIN_INTERNALS__ = previousEvents;
    },
  };
};
