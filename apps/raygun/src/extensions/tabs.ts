export type ExtensionTab = 'installed' | 'browse' | 'repos';

export const EXTENSION_TABS: ExtensionTab[] = ['installed', 'browse', 'repos'];

export const isExtensionTab = (value: unknown): value is ExtensionTab =>
  typeof value === 'string' && (EXTENSION_TABS as string[]).includes(value);
