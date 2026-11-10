/**
 * A menu row's size: 48 px with a mouse; on a touch screen at least 48 px or 7 % of the screen's
 * height, with larger text and icons, so rows are easy to hit and to scroll past.
 */
export const MENU_ROW =
  'min-h-12 pointer-coarse:min-h-[max(3rem,7dvh)] pointer-coarse:gap-4 pointer-coarse:text-[calc(clamp(1rem,4.3vw,1.125rem)*var(--text-scale,1))] pointer-coarse:[&>svg]:size-[calc(22px*var(--ui-scale,1))]';
