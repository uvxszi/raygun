import type { VideoCardStyle } from '@raygun/bindings';
import { Clapperboard } from 'lucide-react';
import type { FC, ReactNode } from 'react';

import { Picture } from '../shell/Picture';

type Props = {
  /** The title's poster (or thumbnail). */
  poster: string | null | undefined;
  /** A wide picture of it (a backdrop): preferred in 16:9. */
  backdrop?: string | null;
  /** The poster's own shape, when the source says (`landscape` fills 16:9 as it is). */
  shape?: string;
  style: VideoCardStyle;
  className?: string;
  /** Over the picture: a muted preview playing. */
  children?: ReactNode;
};

const POSTER_ASPECT: Record<string, string> = {
  landscape: 'aspect-video',
  square: 'aspect-square',
};

/**
 * A video title's picture as its card shows it: in Poster style the poster in its own shape
 * (2:3 unless the source says otherwise); otherwise 16:9, the backdrop when there is one, a wide
 * thumbnail as it is, and a portrait poster whole in the middle of a blurred, darkened fill of
 * itself (so nothing of the poster is cut off).
 */
export const VideoCover: FC<Props> = ({
  poster,
  backdrop,
  shape = 'poster',
  style,
  className = 'w-full rounded-lg',
  children,
}) => {
  const placeholder = <Clapperboard size={28} />;
  if (style === 'poster') {
    return (
      <span className={`relative block overflow-hidden ${className}`}>
        <Picture
          src={poster}
          draggable={false}
          placeholder={placeholder}
          className={`bg-surface text-muted ${POSTER_ASPECT[shape] ?? 'aspect-[2/3]'} w-full`}
        />
        {children}
      </span>
    );
  }
  const wide = backdrop || (shape === 'landscape' ? poster : null);
  if (wide || !poster) {
    return (
      <span className={`relative block overflow-hidden ${className}`}>
        <Picture
          src={wide ?? poster}
          draggable={false}
          placeholder={placeholder}
          className="bg-surface text-muted aspect-video w-full"
        />
        {children}
      </span>
    );
  }
  return (
    <span
      data-testid="video-cover-fill"
      className={`bg-surface relative block aspect-video overflow-hidden ${className}`}
    >
      <img
        src={poster}
        alt=""
        aria-hidden
        draggable={false}
        loading="lazy"
        decoding="async"
        className="absolute inset-0 size-full scale-125 object-cover opacity-60 blur-xl"
      />
      <Picture
        src={poster}
        draggable={false}
        placeholder={placeholder}
        fit="object-contain drop-shadow-lg"
        className="text-muted absolute inset-0 size-full"
      />
      {children}
    </span>
  );
};
