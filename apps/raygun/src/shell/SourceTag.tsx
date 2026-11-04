import type { FC } from 'react';

/**
 * Where something comes from (a source, an addon), as a small bordered tag: kept apart from the
 * line naming who made it, so a source's name never reads as an author.
 */
export const SourceTag: FC<{ name: string; className?: string }> = ({ name, className = '' }) => (
  <span
    data-testid="source-tag"
    className={`border-foreground/20 text-muted inline-flex max-w-full min-w-0 shrink items-center rounded-md border px-1.5 align-middle text-[0.6875rem] leading-4 ${className}`}
  >
    <span className="truncate">{name}</span>
  </span>
);
