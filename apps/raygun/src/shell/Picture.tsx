import { useState } from 'react';
import type { FC, ReactNode, TransitionEvent } from 'react';

import { imageLoading, picturesFade, rememberShown, revealDecoded, wasShown } from './imageMemory';

/** `under` when it's the placeholder that stays under the picture coming in. */
const PLACEHOLDER = 0;

type Top = {
  src: string;
  /** It starts transparent and fades in once decoded: nothing of it has shown this run. */
  fade: boolean;
};

type Layers = {
  /** The address asked for last. */
  asked: string | null;
  /** The picture shown, or on its way: the one asked for, unless it failed. */
  top: Top | null;
  /** What stays under `top` until `top` covers it: the picture shown before, or the placeholder. */
  under: string | typeof PLACEHOLDER | null;
  /** The picture shown before, when none is left to show: it fades out over the placeholder. */
  leaving: string | null;
  /** Addresses that failed to load: the placeholder stands in for them. */
  failed: readonly string[];
};

const topOf = (src: string): Top => ({ src, fade: !wasShown(src) });

const firstLayers = (asked: string | null): Layers => ({
  asked,
  top: asked ? topOf(asked) : null,
  under: null,
  leaving: null,
  failed: [],
});

/** What the frame shows now: a picture, the placeholder, or nothing yet (a picture loading). */
const onScreen = (layers: Layers): string | typeof PLACEHOLDER | null => {
  if (layers.under !== null) return layers.under;
  if (layers.top) return wasShown(layers.top.src) ? layers.top.src : null;
  return PLACEHOLDER;
};

/** The layers once `wanted` is to show (null: no picture, the placeholder). */
const layersFor = (layers: Layers, wanted: string | null): Layers => {
  if (wanted === (layers.top?.src ?? null)) return layers;
  const now = onScreen(layers);
  if (wanted === null) {
    const leaving = typeof now === 'string' && picturesFade() ? now : null;
    return { ...layers, top: null, under: null, leaving };
  }
  // Still on screen under or over the placeholder: it simply stays.
  if (wanted === layers.under || wanted === layers.leaving) {
    return { ...layers, top: { src: wanted, fade: false }, under: null, leaving: null };
  }
  return { ...layers, top: topOf(wanted), under: now, leaving: null };
};

/** Only its own transitions, not one bubbling up from inside. */
const own = (event: TransitionEvent<HTMLElement>) => event.target === event.currentTarget;

type PictureProps = {
  /** The picture's address; none shows `placeholder`. */
  src: string | null | undefined;
  /** The frame: its size and shape (the picture fills it), corners, shadow and background. */
  className?: string;
  /** How each picture fills the frame (object-fit and -position, its own background). */
  fit?: string;
  /** What shows without a picture, or when it fails to load (an icon, a drawn cover). */
  placeholder?: ReactNode;
  /** The placeholder stays under the picture until it has loaded (a drawn book cover). */
  keepPlaceholder?: boolean;
  /** Loads it now rather than when the browser thinks it's near. */
  eager?: boolean;
  alt?: string;
  /** On the picture shown (not the one it replaces). */
  testId?: string;
  /** On the frame. */
  frameTestId?: string;
  /** False: a long press picks a card instead of dragging the picture out (the webview's). */
  draggable?: boolean;
  /** `src` failed to load: the placeholder shows, unless a new address comes. */
  onFailed?: (src: string) => void;
};

/**
 * A picture in a frame of a set size, that fades in once loaded and decoded instead of popping
 * in. A new address keeps the picture on screen under the new one until the new one has decoded,
 * then fades the new one over it (a cover found elsewhere over an empty one, the next song's
 * artwork, an edited cover); one that's gone fades out over the placeholder. A picture already
 * shown this run (shell/imageMemory) shows at once, so going back to a list doesn't flicker; with
 * Reduced or Off motion pictures show at once too. Each picture costs a load handler and one
 * decode: nothing runs per frame, and only the fade's opacity changes, never the layout.
 */
export const Picture: FC<PictureProps> = ({
  src,
  className = '',
  fit = 'object-cover',
  placeholder = null,
  keepPlaceholder = false,
  eager = false,
  alt = '',
  testId,
  frameTestId,
  draggable,
  onFailed,
}) => {
  const asked = src || null;
  const [layers, setLayers] = useState(() => firstLayers(asked));
  let current = layers;
  if (asked !== layers.asked) {
    const wanted = asked && !layers.failed.includes(asked) ? asked : null;
    current = { ...layersFor(layers, wanted), asked };
    setLayers(current);
  }
  const { top, under, leaving } = current;

  /** `from` covers what was under it now. */
  const covered = (from: string) =>
    setLayers((now) =>
      now.top?.src === from && now.under !== null ? { ...now, under: null } : now,
    );

  const failed = (failedSrc: string) => {
    setLayers((now) => {
      if (now.top?.src !== failedSrc) return now;
      const shown = typeof now.under === 'string' && picturesFade() ? now.under : null;
      return {
        ...now,
        top: null,
        under: null,
        leaving: shown,
        failed: [...now.failed, failedSrc],
      };
    });
    onFailed?.(failedSrc);
  };

  // No picture at all: the placeholder alone, without a layer of its own (lists of titles without
  // covers draw nothing more than before).
  if (!top && under === null && !leaving && !keepPlaceholder) {
    return (
      <span aria-hidden data-testid={frameTestId} data-empty="" className={`picture ${className}`}>
        {placeholder}
      </span>
    );
  }
  const loading = top ? imageLoading(top.src, eager) : null;
  const placeholderShown =
    placeholder !== null && (keepPlaceholder || top === null || under === PLACEHOLDER);

  return (
    <span data-testid={frameTestId} className={`picture ${className}`}>
      {top && loading && (
        <img
          key={`picture:${top.src}`}
          src={top.src}
          alt={alt}
          data-testid={testId}
          data-fade={top.fade ? '' : undefined}
          draggable={draggable}
          loading={loading.loading}
          decoding={loading.decoding}
          onLoad={(event) => {
            rememberShown(top.src);
            const image = event.currentTarget;
            if (!top.fade) {
              image.dataset.loaded = '';
              if (under !== null) covered(top.src);
              return;
            }
            // What was under it goes once it's covered: after the fade (styles.css fades only
            // where things may move), or now without one.
            revealDecoded(image, () => {
              if (under !== null && !picturesFade()) covered(top.src);
            });
          }}
          onTransitionEnd={(event) => {
            if (own(event) && under !== null) covered(top.src);
          }}
          onError={() => failed(top.src)}
          className={fit}
        />
      )}
      {leaving && (
        <img
          key={`picture:${leaving}`}
          src={leaving}
          alt=""
          aria-hidden
          data-leaving=""
          draggable={draggable}
          onTransitionEnd={(event) => {
            if (!own(event)) return;
            setLayers((now) => (now.leaving === leaving ? { ...now, leaving: null } : now));
          }}
          className={fit}
        />
      )}
      {typeof under === 'string' && (
        <img
          key={`picture:${under}`}
          src={under}
          alt=""
          aria-hidden
          data-under=""
          draggable={draggable}
          className={fit}
        />
      )}
      {placeholderShown && (
        <span
          key="placeholder"
          aria-hidden
          data-kept={keepPlaceholder ? '' : undefined}
          className="picture-placeholder"
        >
          {placeholder}
        </span>
      )}
    </span>
  );
};
