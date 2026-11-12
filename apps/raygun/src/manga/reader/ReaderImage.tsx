import { ImageOff, LoaderCircle, RotateCw } from 'lucide-react';
import { memo, useState } from 'react';
import type { CSSProperties, FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PageTextOverlay } from '../../readerAi/PageTextOverlay';
import { rememberShown, revealDecoded, wasShown } from '../../shell/imageMemory';
import { pageIdentity } from './drawnPages';
import { usePageAi } from '../../readerAi/useReaderAi';
import { useSteadyPicture } from './steadyPicture';

type ReaderImageProps = {
  url: string;
  index: number;
  className?: string;
  style?: CSSProperties;
  imageRef?: (element: HTMLImageElement | null) => void;
  /** The page's width over its height, once it's loaded. */
  onRatio?: (index: number, ratio: number) => void;
  /** A long press or right click: the page's actions. */
  onMenu: (index: number) => void;
};

/** A retry reaches the source again rather than the failed answer. */
const withAttempt = (url: string, attempt: number) =>
  attempt === 0 ? url : `${url}${url.includes('?') ? '&' : '?'}retry=${attempt}`;

/**
 * One reader page: a spinner while it loads, and when it fails a note and a retry instead of a
 * broken picture. Decoding happens off the main thread. The parent is `relative`, so the status
 * sits over the page's place. With the reader's AI layers on, the page shows as it came until
 * the layers drew it again, and a translation's words lie over it. A picture the layers drew (or
 * the page as it came, once they're off) replaces the one on screen only once it's decoded, on
 * the same element, so the page never goes blank or changes size meanwhile.
 */
const ReaderImageView: FC<ReaderImageProps> = ({
  url,
  index,
  className = '',
  style,
  imageRef,
  onRatio,
  onMenu,
}) => {
  const { t } = useTranslation();
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<'loading' | 'loaded' | 'failed'>('loading');
  const [image, setImage] = useState<HTMLImageElement | null>(null);
  const ai = usePageAi(url);
  // A drawn page asked for wider (zoomed in) is the same page: the one shown stays until the
  // sharper one is ready.
  const src = useSteadyPicture(
    pageIdentity(url),
    ai.src ?? withAttempt(url, attempt),
    state === 'loaded',
  );
  // Decided once for the page: a picture swapped in later (the layers') replaces it on the same
  // element, already shown.
  const [fadeIn] = useState(() => !wasShown(src));
  return (
    <>
      <img
        ref={(element) => {
          setImage(element);
          imageRef?.(element);
        }}
        data-testid="manga-page"
        data-index={index}
        src={src}
        alt={t('manga.pageAlt', { page: index + 1 })}
        decoding="async"
        // A page fades in once decoded (styles.css), unless it has shown before.
        data-fade={fadeIn ? '' : undefined}
        draggable={false}
        onLoad={(event) => {
          setState('loaded');
          const image = event.currentTarget;
          rememberShown(src);
          revealDecoded(image);
          const { naturalWidth, naturalHeight } = image;
          if (naturalWidth > 0 && naturalHeight > 0) onRatio?.(index, naturalWidth / naturalHeight);
        }}
        onError={() => setState('failed')}
        onContextMenu={(event) => {
          event.preventDefault();
          onMenu(index);
        }}
        className={`${className} ${state === 'failed' ? 'invisible' : ''}`}
        style={style}
      />
      {ai.blocks && state === 'loaded' && <PageTextOverlay image={image} blocks={ai.blocks} />}
      {state === 'loading' && (
        <span
          aria-hidden
          className="pointer-events-none absolute inset-0 grid place-items-center text-white/40"
        >
          <LoaderCircle size={28} className="animate-spin" />
        </span>
      )}
      {state === 'failed' && (
        <span
          role="alert"
          className="absolute inset-0 flex flex-col items-center justify-center gap-3 px-6 text-center text-sm text-white/80"
        >
          <ImageOff aria-hidden size={28} />
          {t('reader.pageFailed', { page: index + 1 })}
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              setState('loading');
              setAttempt((count) => count + 1);
            }}
            className="flex items-center gap-2 rounded-full bg-white/15 px-4 py-2 font-semibold text-white"
          >
            <RotateCw aria-hidden size={16} />
            {t('reader.retryPage')}
          </button>
        </span>
      )}
    </>
  );
};

/** Pages re-render only when their own props change, not on every page turn. */
export const ReaderImage = memo(ReaderImageView);
