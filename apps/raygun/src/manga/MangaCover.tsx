import { BookImage, Clapperboard } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { imageLoading } from '../shell/imageMemory';
import { BookCover } from './BookCover';
import type { BookWords } from './bookWords';

type MangaCoverProps = {
  url: string | null | undefined;
  className?: string;
  /** Videos get a clapperboard placeholder instead of a book. */
  video?: boolean;
  /** 16:9, for video sites' thumbnails (M46.45: YouTube, PeerTube). */
  landscape?: boolean;
  /**
   * A book's title and author (`bookWords`): a cover is drawn from them while there's none, and
   * under the book's own until it loads.
   */
  book?: BookWords;
};

/**
 * A cover in the 2:3 shape manga covers have, or a placeholder when it's missing or fails; a
 * book's placeholder is a cover drawn from its title and author ([BookCover]), which also shows
 * while its own cover loads and stays under it (a cover seen before decodes before it's drawn,
 * so nothing flashes).
 */
export const MangaCover: FC<MangaCoverProps> = ({
  url,
  className = '',
  video = false,
  landscape = false,
  book,
}) => {
  const shape = landscape ? 'aspect-video' : 'aspect-[2/3]';
  // Remembered for the address that failed: a new address (a picked cover) is tried at once.
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const failed = failedUrl !== null && failedUrl === url;
  if (book && !landscape) {
    const loading = url && !failed ? imageLoading(url) : null;
    return (
      <span data-testid="book-frame" className={`book-frame block ${shape} ${className}`}>
        {url && loading && (
          <img
            key={url}
            src={url}
            alt=""
            draggable={false}
            loading={loading.loading}
            decoding={loading.decoding}
            onLoad={(event) => {
              loading.onLoad();
              // The drawn cover under it hides (styles.css), transparent corners included.
              event.currentTarget.dataset.loaded = '';
            }}
            onError={() => setFailedUrl(url)}
          />
        )}
        <BookCover title={book.title} author={book.author} />
      </span>
    );
  }
  if (!url || failed) {
    return (
      <span
        aria-hidden
        className={`bg-surface text-muted grid ${shape} place-items-center ${className}`}
      >
        {video ? <Clapperboard size={28} /> : <BookImage size={28} />}
      </span>
    );
  }
  return (
    <img
      src={url}
      alt=""
      // A long press picks the card; the webview would start dragging the picture instead.
      draggable={false}
      {...imageLoading(url)}
      onError={() => setFailedUrl(url)}
      className={`bg-surface ${shape} object-cover ${className}`}
    />
  );
};
