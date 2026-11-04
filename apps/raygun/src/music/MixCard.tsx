import type { MixSummary } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import type { FC, MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { PlaylistCover } from '../collections/PlaylistCover';
import { mixDescription, mixTitle } from './useMadeForYou';

/** A mix as a card: its covers, name and what's in it; it opens the mix (M46.20). */
export const MixCard: FC<{ mix: MixSummary; onMenu?: (event: MouseEvent) => void }> = ({
  mix,
  onMenu,
}) => {
  const { t } = useTranslation();
  const title = mixTitle(t, mix);
  return (
    <li className="w-36 shrink-0 sm:w-44">
      <Link
        to="/music/mix"
        search={{ id: mix.id }}
        data-testid="mix-card"
        onContextMenu={onMenu}
        className="hover:bg-surface flex flex-col gap-2 rounded-xl p-2"
      >
        <span className="relative block">
          <PlaylistCover
            artworkUrls={mix.artworkUrls}
            className="aspect-square w-full rounded-lg shadow-lg"
          />
          {mix.kind === 'daily' && (
            <span className="bg-background/85 absolute bottom-2 left-2 rounded-md px-2 py-0.5 text-xs font-bold">
              {title}
            </span>
          )}
        </span>
        <span className="truncate text-sm font-semibold">{title}</span>
        <span className="text-muted -mt-1.5 line-clamp-2 text-xs">{mixDescription(t, mix)}</span>
      </Link>
    </li>
  );
};
