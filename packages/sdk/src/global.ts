import type { RaygunApi } from './types';

declare global {
  /** The host API, installed by Raygun before the extension's script runs. */
  const raygun: RaygunApi;

  // Declared the way lib.dom does, so this merges cleanly when an extension also has DOM types.
  interface Console {
    /* eslint-disable @typescript-eslint/no-explicit-any */
    log(...data: any[]): void;
    info(...data: any[]): void;
    debug(...data: any[]): void;
    warn(...data: any[]): void;
    error(...data: any[]): void;
    /* eslint-enable @typescript-eslint/no-explicit-any */
  }
  var console: Console;

  // QuickJS's own base64, declared as lib.dom does: text of bytes 0-255 to base64 and back.
  function atob(data: string): string;
  function btoa(data: string): string;
}

export {};
