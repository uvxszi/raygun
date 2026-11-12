import { Podcast } from 'lucide-react';
import type { FC } from 'react';
import { Picture } from '../shell/Picture';

/** Square podcast artwork, or a placeholder when it's missing or fails; it fades in. */
export const PodcastArt: FC<{ url: string | null | undefined; className?: string }> = ({
  url,
  className = '',
}) => (
  <Picture
    src={url}
    // A long press picks a show or a book; the webview would start dragging the picture.
    draggable={false}
    placeholder={<Podcast size={28} />}
    className={`bg-surface text-muted aspect-square ${className}`}
  />
);
