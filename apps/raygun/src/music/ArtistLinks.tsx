import { Link, useNavigate } from '@tanstack/react-router';
import { Fragment } from 'react';
import type { FC, KeyboardEvent, MouseEvent } from 'react';

import { useCoarsePointer } from '../shell/useWideScreen';
import { creditedArtists } from './artists';

const LINK = 'hover:text-foreground cursor-pointer hover:underline';

/**
 * A credit as links to each artist's page (M46.8): "Daft Punk, Pharrell Williams" leads to both.
 * `nested` is for credits inside a row's button (a real link can't sit in a button): link-role
 * spans that open the page on click or Enter without playing the row. Each link is only its
 * name's text, so it never reaches into the title's line.
 */
export const ArtistLinks: FC<{
  credit: string | null | undefined;
  className?: string;
  nested?: boolean;
  /**
   * The artist line of a song row: on a touch screen the names are plain text, so a tap anywhere
   * on the row plays the song (Go to artist is in the row's menu), as Spotify does.
   */
  row?: boolean;
}> = ({ credit, className = '', nested = false, row = false }) => {
  const navigate = useNavigate();
  const touch = useCoarsePointer();
  const artists = creditedArtists(credit);
  if (artists.length === 0) return null;
  if (row && touch) return <span className={className}>{credit}</span>;
  const open = (name: string) => (event: MouseEvent | KeyboardEvent) => {
    event.stopPropagation();
    event.preventDefault();
    void navigate({ to: '/artist', search: { name } });
  };
  return (
    <span className={className}>
      {artists.map((name, index) => (
        <Fragment key={`${name}-${index}`}>
          {index > 0 && ', '}
          {nested ? (
            <span
              role="link"
              tabIndex={0}
              onClick={open(name)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') open(name)(event);
              }}
              className={LINK}
            >
              {name}
            </span>
          ) : (
            <Link
              to="/artist"
              search={{ name }}
              onClick={(event) => event.stopPropagation()}
              className={LINK}
            >
              {name}
            </Link>
          )}
        </Fragment>
      ))}
    </span>
  );
};
