import { commands } from '@raygun/bindings';
import type { DetailTarget } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Disc3 } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { imageLoading } from '../shell/imageMemory';

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

/** Album art, or a placeholder when there is none or it fails to load. */
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
  if (!shown || failedUrls.includes(shown)) {
    return (
      <span
        aria-hidden
        className={`bg-surface text-muted grid aspect-square place-items-center ${className}`}
      >
        <Icon size={iconSize} />
      </span>
    );
  }
  return (
    <img
      src={shown}
      data-testid="artwork-image"
      alt=""
      {...imageLoading(shown, eager)}
      onError={() => setFailedUrls((failed) => [...failed, shown])}
      className={`bg-surface aspect-square object-cover ${className}`}
    />
  );
};
