// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From lib/blocklist.ts: the entries come from two text settings instead of NouTube's own list.

/** Channels and title words whose videos are hidden, each normalized (see `normalizeBlocklistValue`). */
export type Blocklist = {
  channels: string[];
  keywords: string[];
};

export const emptyBlocklist = (): Blocklist => ({ channels: [], keywords: [] });

export const isBlocklistEmpty = (blocklist: Blocklist | undefined) =>
  !blocklist || (!blocklist.channels.length && !blocklist.keywords.length);

/**
 * A channel's name, handle or address, or a title word, as it is compared: lowercase, the
 * address and `@` left out, so `https://www.youtube.com/@Name`, `@name` and `Name` all match.
 */
export const normalizeBlocklistValue = (value: string) =>
  value
    .trim()
    .replace(/^https?:\/\/(www\.|m\.)?youtube\.com\//i, '')
    .replace(/^\/+/, '')
    .replace(/^(c|channel|user)\//i, '')
    .replace(/^\/+/, '')
    .replace(/^@+/, '')
    .replace(/\s+/g, ' ')
    .toLocaleLowerCase();

/** The entries of a setting: separated by commas or new lines, normalized, without repeats. */
export const parseBlocklistEntries = (text: string): string[] => {
  const entries = new Set<string>();
  for (const part of text.split(/[,\n]/)) {
    const value = normalizeBlocklistValue(part);
    if (value) entries.add(value);
  }
  return [...entries];
};

export const blocklistFromSettings = (channels: string, keywords: string): Blocklist => ({
  channels: parseBlocklistEntries(channels),
  keywords: parseBlocklistEntries(keywords),
});

/** A channel matches only as a whole, so hiding "Google" leaves "Google Developers" alone. */
export const blocklistChannelMatches = (text: string | undefined, channels: string[]) => {
  if (!text) return false;
  const normalized = normalizeBlocklistValue(text);
  return Boolean(normalized) && channels.some((value) => normalized === value);
};

export const blocklistChannelsMatch = (texts: (string | undefined)[], channels: string[]) =>
  texts.some((text) => blocklistChannelMatches(text, channels));

/** A title word matches anywhere in the title. */
export const blocklistTextMatches = (text: string | undefined, keywords: string[]) => {
  if (!text) return false;
  const normalized = normalizeBlocklistValue(text);
  return Boolean(normalized) && keywords.some((value) => normalized.includes(value));
};

export const blocklistTextsMatch = (texts: (string | undefined)[], keywords: string[]) =>
  texts.some((text) => blocklistTextMatches(text, keywords));
