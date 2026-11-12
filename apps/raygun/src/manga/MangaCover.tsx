import { BookImage, Clapperboard } from 'lucide-react';
import type { FC } from 'react';
import { Picture } from '../shell/Picture';
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
 * while its own cover loads and stays under it until the cover has faded in over it. A cover
 * seen before shows at once; a new one fades in over the one before (shell/Picture).
 */
export const MangaCover: FC<MangaCoverProps> = ({
  url,
  className = '',
  video = false,
  landscape = false,
  book,
}) => {
  const shape = landscape ? 'aspect-video' : 'aspect-[2/3]';
  if (book && !landscape) {
    return (
      <Picture
        src={url}
        frameTestId="book-frame"
        // A long press picks the card; the webview would start dragging the picture instead.
        draggable={false}
        keepPlaceholder
        placeholder={<BookCover title={book.title} author={book.author} />}
        className={`book-frame ${shape} ${className}`}
      />
    );
  }
  return (
    <Picture
      src={url}
      draggable={false}
      placeholder={video ? <Clapperboard size={28} /> : <BookImage size={28} />}
      className={`bg-surface text-muted ${shape} ${className}`}
    />
  );
};
