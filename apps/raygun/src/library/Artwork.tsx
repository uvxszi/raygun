import { commands } from '@raygun/bindings';
import type { DetailTarget } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Disc3 } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { Picture } from '../shell/Picture';

type ArtworkProps = {
  url: string | null | undefined;
  className?: string;
  iconSize?: number;
  /**
   * What the picture is of (M46.40): when its own is missing or fails, another source's cover of
   * the same song, album or artist is used, found once and remembered.
   */
  fallback?: DetailTarget | null;
  /** The placeholder's icon (a disc unless it's someone, say). */
  icon?: LucideIcon;
  /** Load it now rather than when the browser thinks it's near (a cover made of four). */
  eager?: boolean;
};

/**
 * Album art, or a placeholder when there is none or it fails to load. It fades in, and a new
 * cover (the next song's, one found elsewhere, an edited one) fades in over the one before
 * (shell/Picture).
 */
export const Artwork: FC<ArtworkProps> = ({
  url,
  className = '',
  iconSize = 32,
  fallback,
  icon: Icon = Disc3,
  eager = false,
}) => {
  // Remembered per address: a new address (a picked cover) is tried at once.
  const [failedUrls, setFailedUrls] = useState<string[]>([]);
  const ownFailed = !url || failedUrls.includes(url);
  const other = useQuery({
    queryKey: ['coverFallback', JSON.stringify(fallback ?? null)],
    queryFn: () => unwrapResult(commands.coverFallback(fallback as DetailTarget)),
    enabled: ownFailed && Boolean(fallback),
    staleTime: Infinity,
    retry: false,
  });
  const shown = ownFailed ? (other.data ?? null) : url;
  return (
    <Picture
      src={shown && !failedUrls.includes(shown) ? shown : null}
      testId="artwork-image"
      eager={eager}
      onFailed={(failed) => setFailedUrls((list) => [...list, failed])}
      placeholder={<Icon size={iconSize} />}
      className={`bg-surface text-muted aspect-square ${className}`}
    />
  );
};
