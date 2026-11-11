// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/clickbait.ts, itself adapted from
// https://github.com/pietervanheijningen/clickbait-remover-for-youtube (MIT).
//
// Thumbnails chosen by the uploader are swapped for the frames YouTube takes of every video
// itself (hq1, hq2, hq3: from the start, the middle and the end), by changing the picture's
// address as the page sets it.

/** The setting's choices, by the frame each one shows; the first keeps the uploader's thumbnail. */
const TARGETS: Record<string, string> = {
  'Frame from the start': 'hq1',
  'Frame from the middle': 'hq2',
  'Frame from the end': 'hq3',
};

/** The frame a choice of the setting stands for; undefined for the uploader's thumbnail. */
export const thumbnailTarget = (choice: string): string | undefined => TARGETS[choice];

const QUALITIES = 'default|mqdefault|hqdefault|sddefault|hq720|hq1|hq2|hq3|maxresdefault';
const THUMB_RE = new RegExp(
  `^(https?://i\\d?\\.ytimg\\.com/(?:vi|vi_webp)/[^/]+/)(${QUALITIES})(_custom_\\d+)?(\\.[a-zA-Z0-9]+)(.*)$`,
);
const THUMB_IN_STYLE_RE = new RegExp(
  `(https?://i\\d?\\.ytimg\\.com/(?:vi|vi_webp)/[^/]+/)(${QUALITIES})(_custom_\\d+)?(\\.[a-zA-Z0-9]+)`,
  'g',
);

function transformUrl(url: string, target: string): string {
  const match = THUMB_RE.exec(url);
  if (!match || match[2] === target) return url;
  return `${match[1]}${target}${match[4]}${match[5]}`;
}

const transformStyle = (value: string, target: string) =>
  value.replace(THUMB_IN_STYLE_RE, `$1${target}$4`);

export function installClickbaitThumbnails(target: string) {
  try {
    const descriptor = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'src');
    const setSource = descriptor?.set;
    const getSource = descriptor?.get;
    if (setSource && getSource) {
      Object.defineProperty(HTMLImageElement.prototype, 'src', {
        configurable: true,
        enumerable: true,
        get(this: HTMLImageElement) {
          return getSource.call(this);
        },
        set(this: HTMLImageElement, value: unknown) {
          setSource.call(this, typeof value === 'string' ? transformUrl(value, target) : value);
        },
      });
    }
  } catch (error) {
    console.warn('[YouTube enhancements] thumbnails:', error);
  }

  try {
    const setAttribute = Element.prototype.setAttribute;
    Element.prototype.setAttribute = function (this: Element, name: string, value: string) {
      if (this instanceof HTMLImageElement && name === 'src' && typeof value === 'string') {
        return setAttribute.call(this, name, transformUrl(value, target));
      }
      return setAttribute.call(this, name, value);
    };
  } catch (error) {
    console.warn('[YouTube enhancements] thumbnails:', error);
  }

  // Pictures already in the page, and backgrounds set through styles.
  const sweep = () => {
    document.querySelectorAll('img').forEach((image) => {
      const current = image.getAttribute('src');
      if (!current) return;
      const next = transformUrl(current, target);
      if (next !== current) image.setAttribute('src', next);
    });
    document.querySelectorAll<HTMLElement>('[style*="ytimg.com"]').forEach((element) => {
      const style = element.getAttribute('style');
      if (!style) return;
      const next = transformStyle(style, target);
      if (next !== style) element.setAttribute('style', next);
    });
  };

  if (document.body) sweep();
  document.addEventListener('DOMContentLoaded', sweep);
  setTimeout(sweep, 1000);
  setTimeout(sweep, 3000);
}
