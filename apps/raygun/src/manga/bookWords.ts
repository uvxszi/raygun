/** FNV-1a (32 bits): the same title always gets the same colours. */
const hash = (text: string) => {
  let value = 0x811c9dc5;
  for (let index = 0; index < text.length; index += 1) {
    value ^= text.charCodeAt(index);
    value = Math.imul(value, 0x01000193);
  }
  return value >>> 0;
};

/**
 * A book's colours from its title (manga/BookCover): a deep gradient from one hue into a
 * neighbouring one, dark enough for light lettering, and a lighter accent for the rule and frame.
 */
export const bookCoverLook = (title: string) => {
  const seed = hash(title.trim().toLowerCase());
  const hue = seed % 360;
  const drift = ((seed >>> 9) % 50) - 25;
  const saturation = 32 + ((seed >>> 15) % 28);
  const light = 26 + ((seed >>> 21) % 10);
  const end = (hue + drift + 360) % 360;
  return {
    background: `linear-gradient(160deg, hsl(${hue} ${saturation}% ${light}%), hsl(${end} ${saturation + 6}% ${light - 13}%))`,
    accent: `hsl(${(hue + 40) % 360} 70% 78%)`,
  };
};

/** How long a title is, for the size it's set in (`.book-cover-title[data-length]`). */
export const titleLength = (title: string) =>
  title.length <= 14
    ? 'short'
    : title.length <= 32
      ? 'medium'
      : title.length <= 60
        ? 'long'
        : 'longer';

/** The words a book's drawn cover shows. */
export type BookWords = { title: string; author?: string | null };

/**
 * The authors a cached answer names for a title (a listing's card, a title page, a library
 * entry), when it names any.
 */
export const authorsIn = (record: Record<string, unknown>): string[] | undefined => {
  if (!Array.isArray(record.authors)) return undefined;
  const names = record.authors.filter(
    (name): name is string => typeof name === 'string' && name.trim() !== '',
  );
  return names.length > 0 ? names : undefined;
};

/** A title seen again elsewhere keeps what it had, and takes the authors it lacked. */
export const withAuthors = <T extends { authors?: readonly string[] }>(kept: T, found: T): T =>
  kept.authors?.length || !found.authors?.length ? kept : { ...kept, authors: found.authors };

/**
 * The drawn cover's words for a title of `medium`: books (`novel`) get one, other media none.
 * `authors` may be a list, joined as the cover prints it.
 */
export const bookWords = (
  medium: string | null | undefined,
  title: string,
  authors?: readonly string[] | string | null,
): BookWords | undefined =>
  medium === 'novel'
    ? { title, author: typeof authors === 'string' ? authors : authors?.join(', ') || null }
    : undefined;
