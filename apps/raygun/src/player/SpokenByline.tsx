import type { SpokenItem } from '@raygun/bindings';
import { Link, useNavigate } from '@tanstack/react-router';
import type { FC, KeyboardEvent, MouseEvent } from 'react';

import { spokenName } from './spoken';

const LINK = 'hover:text-foreground cursor-pointer hover:underline';

/**
 * Who a spoken item is by (M46.32): a book's author, or a podcast's show, linking to the book's
 * or the show's page rather than to a music artist. `nested` is for text inside a button.
 */
export const SpokenByline: FC<{
  spoken: SpokenItem;
  className?: string;
  nested?: boolean;
}> = ({ spoken, className = '', nested = false }) => {
  const navigate = useNavigate();
  const book = spoken.kind === 'audiobook';
  const name = spokenName(spoken);
  const to = book ? '/audiobook' : '/podcast';
  const search = { id: spoken.podcastId };
  const open = (event: MouseEvent | KeyboardEvent) => {
    event.stopPropagation();
    event.preventDefault();
    void navigate({ to, search });
  };
  return (
    <span className={className} data-testid="spoken-byline">
      {nested ? (
        <span
          role="link"
          tabIndex={0}
          onClick={open}
          onKeyDown={(event) => {
            if (event.key === 'Enter') open(event);
          }}
          className={LINK}
        >
          {name}
        </span>
      ) : (
        <Link to={to} search={search} className={LINK}>
          {name}
        </Link>
      )}
    </span>
  );
};
