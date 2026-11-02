/**
 * The one fixed-position container holding the YouTube iframe. An iframe reloads whenever it is
 * moved in the DOM, so instead of moving it the dock is repositioned: over the player sheet's
 * video area while that is visible, otherwise off-screen (still playing).
 */

const OFFSCREEN = { left: -10_000, top: 0, width: 320, height: 180 };

let dock: HTMLDivElement | null = null;

export const createVideoDock = (): { container: HTMLDivElement; frame: HTMLIFrameElement } => {
  const container = document.createElement('div');
  container.dataset.testid = 'video-dock';
  Object.assign(container.style, {
    position: 'fixed',
    zIndex: '60',
    overflow: 'hidden',
    borderRadius: '16px',
    // Clickable only while it shows (place): YouTube's own play/pause and links (M46.8).
    pointerEvents: 'none',
    background: '#000',
  });
  const frame = document.createElement('iframe');
  frame.title = 'YouTube player';
  frame.allow = 'autoplay; encrypted-media; picture-in-picture';
  Object.assign(frame.style, { width: '100%', height: '100%', border: '0' });
  container.append(frame);
  document.body.append(container);
  dock = container;
  hideVideo();
  return { container, frame };
};

export const removeVideoDock = () => {
  dock?.remove();
  dock = null;
};

/** How much of the video's top and bottom its surroundings hide, in px. */
export type VideoClip = { top: number; bottom: number };

export const showVideoAt = (
  rect: Pick<DOMRect, 'left' | 'top' | 'width' | 'height'>,
  clip: VideoClip = { top: 0, bottom: 0 },
) => {
  place(rect, true, clip);
};

export const hideVideo = () => {
  place(OFFSCREEN, false);
};

const place = (
  { left, top, width, height }: Pick<DOMRect, 'left' | 'top' | 'width' | 'height'>,
  shown: boolean,
  clip: VideoClip = { top: 0, bottom: 0 },
) => {
  if (!dock) {
    return;
  }
  Object.assign(dock.style, {
    left: `${left}px`,
    top: `${top}px`,
    width: `${width}px`,
    height: `${height}px`,
    pointerEvents: shown ? 'auto' : 'none',
    clipPath: clip.top > 0 || clip.bottom > 0 ? `inset(${clip.top}px 0 ${clip.bottom}px 0)` : '',
  });
};
