/**
 * Sizes of what you tap in Connect and the device lists, on phones and desktop alike: device rows
 * at least 56 px tall and other rows 48 px, with room around their words, and buttons at least
 * 44 px, so a device is easy to hit on a phone held in one hand.
 */

/** A device you tap: icon, name and a line under it. */
export const TAP_ROW = 'flex min-h-14 w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left';

/** A one-line choice under a device (where its sound comes out). */
export const TAP_OPTION =
  'flex min-h-12 w-full items-center gap-3 rounded-xl px-3 py-2 text-left text-base';

/** A pill button beside or under a device (Play there, Sync now, Allow). */
export const TAP_BUTTON =
  'flex min-h-11 shrink-0 items-center justify-center gap-2 rounded-full px-4 text-sm font-semibold disabled:opacity-50';

/** A button with only an icon (Forget, Unlink). */
export const TAP_ICON = 'grid size-11 shrink-0 place-items-center rounded-full disabled:opacity-50';

/** A device's name in a row, and the line under it. */
export const TAP_NAME = 'block truncate text-base font-medium';
export const TAP_NOTE = 'text-muted block truncate text-sm';
