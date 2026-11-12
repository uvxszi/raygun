import { Check } from 'lucide-react';
import { memo } from 'react';
import type { FC, SyntheticEvent } from 'react';

import { imageLoading, rememberShown, revealDecoded, wasShown } from '../shell/imageMemory';

/**
 * The source's letter shows until its icon has loaded and faded in over it (styles.css, a
 * `.picture` frame), and stays when the icon doesn't load (marked on the element itself: tiles
 * keep no state).
 */
const iconShown = (event: SyntheticEvent<HTMLImageElement>) => {
  const image = event.currentTarget;
  const src = image.getAttribute('src');
  if (src) rememberShown(src);
  revealDecoded(image);
};
const iconBroken = (event: SyntheticEvent<HTMLImageElement>) => {
  event.currentTarget.style.display = 'none';
};

export type SourceTileProps = {
  id: string;
  name: string;
  icon: string | null;
  /** For a source rated for adults: its 18+ mark and what the mark says to a screen reader. */
  adultMark: string | null;
  adultLabel: string | null;
  on: boolean;
  /** Its place in the grid, for moving between tiles with the arrow keys. */
  row: number;
  col: number;
  /** The grid's one stop for Tab; the arrow keys reach the other tiles. */
  tabStop: boolean;
};

/**
 * One source's toggle in Custom sources' grid: its icon (or first letter) and name, tinted and
 * checked while on. It has no handlers or hooks of its own: the grid's one click and key
 * listener switches it, so a tile is drawn again only when what it shows changes.
 */
const SourceTileView: FC<SourceTileProps> = ({
  id,
  name,
  icon,
  adultMark,
  adultLabel,
  on,
  row,
  col,
  tabStop,
}) => (
  <button
    type="button"
    role="switch"
    aria-checked={on}
    aria-label={adultLabel ? `${name}, ${adultLabel}` : name}
    title={name}
    tabIndex={tabStop ? 0 : -1}
    data-cell=""
    data-source={id}
    data-row={row}
    data-col={col}
    data-testid="source-tile"
    className={`focus-visible:outline-primary relative flex min-w-0 flex-col items-center gap-1 rounded-lg border px-0.5 pt-2 pb-1.5 text-center transition-colors focus-visible:outline-2 focus-visible:outline-offset-1 ${
      on
        ? 'border-primary/50 bg-primary/12 text-foreground'
        : 'border-foreground/10 bg-background text-muted hover:border-foreground/30'
    }`}
  >
    <span
      aria-hidden
      className={`picture size-8 shrink-0 rounded-lg ${on ? '' : 'opacity-50 grayscale'}`}
    >
      {icon && (
        <img
          src={icon}
          alt=""
          // Only the rows near the screen are drawn; their icons load as they come (at once
          // when seen before, without fading in again).
          loading={imageLoading(icon).loading}
          decoding={wasShown(icon) ? 'sync' : 'async'}
          data-fade={wasShown(icon) ? undefined : ''}
          draggable={false}
          onLoad={iconShown}
          onError={iconBroken}
          className="rounded-lg object-contain"
        />
      )}
      <span data-kept="" className="picture-placeholder">
        <span className="bg-primary/15 text-primary grid size-full place-items-center rounded-lg text-sm font-semibold">
          {name.slice(0, 1).toUpperCase()}
        </span>
      </span>
    </span>
    <span className="line-clamp-2 min-h-[2lh] w-full text-2xs leading-tight tracking-tight break-words hyphens-auto">
      {name}
    </span>
    <span
      aria-hidden
      className={`absolute top-1 right-1 grid size-4 place-items-center rounded-full ${
        on ? 'bg-primary text-background' : 'border-muted/50 border'
      }`}
    >
      {on && <Check size={10} strokeWidth={3.5} />}
    </span>
    {adultMark && (
      <span
        aria-hidden
        className="border-muted/60 text-muted absolute top-1 left-1 rounded-sm border px-0.5 text-[0.5rem] leading-3 font-bold tabular-nums"
      >
        {adultMark}
      </span>
    )}
  </button>
);

export const SourceTile = memo(SourceTileView);
