/** Browse → Extensions' pills: every repository's extensions, then by whether they're here. */
export type ExtensionList = 'all' | 'installed' | 'notInstalled' | 'updates';

export const EXTENSION_LISTS: ExtensionList[] = ['all', 'installed', 'notInstalled', 'updates'];

export const isExtensionList = (value: unknown): value is ExtensionList =>
  typeof value === 'string' && (EXTENSION_LISTS as string[]).includes(value);
