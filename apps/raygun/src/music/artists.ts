/** Words that join artists in a credit ("A, B & C feat. D"), as Rust's `credited_artists`. */
const SEPARATORS = /,\s+|\s+&\s+|\s+feat\.?\s+|\s+ft\.?\s+|\s+featuring\s+|\s+x\s+|\s+and\s+/i;

/** Double quotes a source, or a screenshot read as text, may leave around a name. */
const QUOTES = new Set(['"', '“', '”', '„', '‟', '«', '»']);

/**
 * A credit or a name without the quotes left around it, as Rust's `tidy_credit`: `"Name"` and a
 * lone `"Name` lose them, while quotes that are part of a name (`"Weird Al" Yankovic`) stay.
 */
export const tidyCredit = (credit: string): string => {
  const text = credit.trim();
  const quotes = [...text].filter((char) => QUOTES.has(char)).length;
  const first = QUOTES.has(text.charAt(0));
  const last = QUOTES.has(text.charAt(text.length - 1));
  if (quotes === 2 && first && last) return text.slice(1, -1).trim();
  if (quotes === 1 && first) return text.slice(1).trim();
  if (quotes === 1 && last) return text.slice(0, -1).trim();
  return text;
};

/**
 * The artists named in a credit, in order (M46.8). A name that is itself a pair ("Simon &
 * Garfunkel") comes apart here; the artist page still finds songs credited to the pair.
 */
export const creditedArtists = (credit: string | null | undefined): string[] =>
  (credit ?? '')
    .split(SEPARATORS)
    .map((part) => tidyCredit(part.replace(/^[(\s]+|[)\s]+$/g, '')))
    .filter(Boolean);
