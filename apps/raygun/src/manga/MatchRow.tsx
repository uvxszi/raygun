import { Link } from '@tanstack/react-router';
import type { FC, KeyboardEvent, ReactNode } from 'react';

/** Where a match opens: a page by its route, or something to do first (store a feed, then go). */
export type MatchOpen = { to: string; search: Record<string, string> } | (() => void);

/**
 * The cover or the title, opening the match. Both stay inline boxes, so a long title flows round
 * the ⋮ beside its first line and takes the row's whole width below it.
 */
const Opener: FC<{
  open: MatchOpen;
  className: string;
  /** The cover repeats the title's link: for touch, kept from the keys and screen readers. */
  repeat?: boolean;
  children: ReactNode;
}> = ({ open, className, repeat = false, children }) => {
  const hidden = repeat ? ({ tabIndex: -1, 'aria-hidden': true } as const) : {};
  if (typeof open !== 'function') {
    return (
      <Link to={open.to as never} search={open.search as never} className={className} {...hidden}>
        {children}
      </Link>
    );
  }
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      open();
    }
  };
  return (
    <span
      role={repeat ? undefined : 'link'}
      tabIndex={repeat ? -1 : 0}
      aria-hidden={repeat || undefined}
      onClick={open}
      onKeyDown={onKeyDown}
      className={`cursor-pointer ${className}`}
    >
      {children}
    </span>
  );
};

/**
 * One match in Find in other sources: its cover, then its title in full (wrapping, never cut)
 * with what it is and where it's from under it, its chips, and its actions (Add…) under those.
 * The ⋮ sits at the right of the title's first line only, so the text keeps the row's width
 * below it.
 */
export const MatchRow: FC<{
  open: MatchOpen;
  cover: ReactNode;
  title: string;
  /** Lines under the title: the source, the author… (each wraps round the ⋮ too). */
  lines?: ReactNode[];
  /** What it has (chips), or that it's being checked. */
  chips?: ReactNode;
  actions?: ReactNode;
  /** The ⋮ button. */
  menu?: ReactNode;
  /** Why an action failed. */
  issue?: ReactNode;
}> = ({ open, cover, title, lines, chips, actions, menu, issue }) => (
  <div className="flex flex-col gap-1 py-2">
    <div className="flex items-start gap-3">
      <Opener open={open} repeat className="block w-10 shrink-0">
        {cover}
      </Opener>
      <div className="min-w-0 flex-1" data-testid="match-text">
        {menu && <div className="float-right -mt-1.5 -mr-2 ml-1">{menu}</div>}
        <Opener
          open={open}
          className="text-sm leading-snug font-medium wrap-anywhere hover:underline"
        >
          {title}
        </Opener>
        {lines
          ?.filter((line) => line !== null && line !== undefined && line !== '')
          .map((line, index) => (
            <span key={index} className="text-muted mt-0.5 block text-xs wrap-anywhere">
              {line}
            </span>
          ))}
        {chips && <div className="mt-1">{chips}</div>}
        {actions && <div className="mt-1.5 flex flex-wrap items-center gap-2">{actions}</div>}
      </div>
    </div>
    {issue}
  </div>
);

/** A match's action as a small pill (Add, Follow, Open). */
export const MATCH_ACTION =
  'bg-background hover:bg-primary/15 flex shrink-0 items-center gap-1 rounded-full px-3 py-1.5 text-xs font-semibold disabled:opacity-60';
