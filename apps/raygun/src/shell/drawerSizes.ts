/** The phone's quick access drawer with names: this wide (rem), at most this much of the screen (vw). */
export const DRAWER_WIDE_REM = 18;
export const DRAWER_WIDE_VW = 82;
/** With covers only (rem). */
export const DRAWER_NARROW_REM = 5.5;

/** The list's side padding in the drawer (rem, both sides): `px-2` with names, `px-1` without. */
const LIST_PAD_REM = { wide: 1, narrow: 0.5 } as const;
/** A row's padding around its cover (rem, both sides): `p-1.5`. */
const ROW_PAD_REM = 0.75;
/** With names, a cover takes this share of the list's width, between these sizes (rem). */
const COVER_SHARE = 0.21;
const COVER_MIN_REM = 3;
const COVER_MAX_REM = 4.5;

/** The drawer's list width (px) before it's laid out: as wide as the drawer, less its padding. */
export const drawerListWidth = (expanded: boolean, viewportPx: number, remPx: number) =>
  expanded
    ? Math.min(DRAWER_WIDE_REM * remPx, (DRAWER_WIDE_VW / 100) * viewportPx) -
      LIST_PAD_REM.wide * remPx
    : (DRAWER_NARROW_REM - LIST_PAD_REM.narrow) * remPx;

/**
 * How large a cover is in the drawer (px), from its list's width: with covers only, as wide as a
 * row holds; with names, a fifth of the list (3 to 4.5 rem), leaving the name room for two lines
 * beside it and its ⋮.
 */
export const drawerCover = (expanded: boolean, listPx: number, remPx: number) =>
  expanded
    ? Math.round(
        Math.min(COVER_MAX_REM * remPx, Math.max(COVER_MIN_REM * remPx, listPx * COVER_SHARE)),
      )
    : Math.max(Math.round(2.5 * remPx), Math.floor(listPx - ROW_PAD_REM * remPx));
