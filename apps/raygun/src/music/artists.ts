/** Words that join artists in a credit ("A, B & C feat. D"), as Rust's `credited_artists`. */
const SEPARATORS = /,\s+|\s+&\s+|\s+feat\.?\s+|\s+ft\.?\s+|\s+featuring\s+|\s+x\s+|\s+and\s+/i;

/**
 * The artists named in a credit, in order (M46.8). A name that is itself a pair ("Simon &
 * Garfunkel") comes apart here; the artist page still finds songs credited to the pair.
 */
export const creditedArtists = (credit: string | null | undefined): string[] =>
  (credit ?? '')
    .split(SEPARATORS)
    .map((part) => part.replace(/^[(\s]+|[)\s]+$/g, '').trim())
    .filter(Boolean);
