// What Raygun gives converted Nuclear plugins beyond the `raygun` API (raygun-ext-host `js.rs`):
// pages parsed and queried in Rust (the documents stay there, so a page costs the plugin's heap
// nothing), digests and HMACs, random bytes, and the plugin's settings on its page. The hook is
// taken once, before the plugin runs, and removed from the global scope.

export type Natives = {
  /** A document operation (`load`, `select`, `info`, `text`...): a JSON envelope. */
  dom(op: string, args: string): string;
  /** `{ op: 'digest' | 'hmac', ... }` with base64 bytes: a JSON envelope of base64. */
  crypto(args: string): string;
  /** `count` random bytes (at most 65536): a JSON envelope of base64. */
  random(count: number): string;
  /**
   * Adds a setting to the plugin's page, as Raygun's settings are written (`key`, `type`,
   * `label`, `options`, `default`, `required`): a JSON envelope.
   */
  registerSetting?(json: string): string;
};

/** Reads a native's `{ value }` or `{ error }` answer. */
export const unwrap = <T>(text: string): T => {
  const envelope = JSON.parse(text) as { value?: T; error?: string };
  if (envelope.error !== undefined) throw new Error(envelope.error);
  return envelope.value as T;
};

let taken: Natives | null = null;

/** The natives taken at start; null where Raygun gives none (tests). */
export const natives = (): Natives | null => taken;

/** Takes the natives from the global scope; null where Raygun gives none (tests). */
export const takeNatives = (scope: Record<string, unknown>): Natives | null => {
  const found = (scope.__rgNuclear as Natives | undefined) ?? null;
  delete scope.__rgNuclear;
  taken = found;
  return found;
};
