// Links to an app's stream proxy (`http://127.0.0.1:<port>/cover/…`) can't load in a page served
// from elsewhere: the page shows them as its own `__remote/…` paths, which its service worker
// answers through the linked app (media.ts). What the page keeps or sends to the app goes back to
// the app's own form, so stored and synced data reads the same on every device.

/** The proxy's routes (raygun-app web_remote.rs `ROUTES`). */
const ROUTES = 'stream|image|cover|subtitle|art|local|font|pdf|torrent|media';
const LOOPBACK = new RegExp(`^http://(?:127\\.0\\.0\\.1|localhost):\\d+/((?:${ROUTES})/.*)$`, 's');
const LOOPBACK_IN_TEXT = new RegExp(
  `http://(?:127\\.0\\.0\\.1|localhost):\\d+/(?=(?:${ROUTES})/)`,
  'g',
);

/** Where the page's own paths for an app's links start, under the page's base. */
export const REMOTE_DIR = '__remote/';

/** Maps every string in `value`, keeping objects that didn't change as they were. */
const mapStrings = (value: unknown, map: (text: string) => string): unknown => {
  if (typeof value === 'string') return map(value);
  if (Array.isArray(value)) {
    let changed = false;
    const out = value.map((item) => {
      const next = mapStrings(item, map);
      if (next !== item) changed = true;
      return next;
    });
    return changed ? out : value;
  }
  if (
    value !== null &&
    typeof value === 'object' &&
    Object.getPrototypeOf(value) === Object.prototype
  ) {
    let changed = false;
    const out: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value)) {
      const next = mapStrings(item, map);
      if (next !== item) changed = true;
      out[key] = next;
    }
    return changed ? out : value;
  }
  return value;
};

/** `base` is the page's base address (`https://…/raygun/`). */
export const toPageLinks = <T>(value: T, base: string): T =>
  mapStrings(value, (text) => {
    if (!text.startsWith('http://')) return text;
    const match = LOOPBACK.exec(text);
    return match ? `${base}${REMOTE_DIR}${match[1]}` : text;
  }) as T;

/** The page's paths back to the app's own links, while its proxy's address is known. */
export const toAppLinks = <T>(value: T, base: string, proxy: string | null): T => {
  if (!proxy) return value;
  const prefix = `${base}${REMOTE_DIR}`;
  return mapStrings(value, (text) =>
    text.startsWith(prefix) ? `${proxy}/${text.slice(prefix.length)}` : text,
  ) as T;
};

/** A playlist's text with its links to the proxy made the page's (HLS, DASH). */
export const pageLinksInText = (text: string, base: string) =>
  text.replace(LOOPBACK_IN_TEXT, `${base}${REMOTE_DIR}`);

/** The proxy path a page path names, or null for other addresses. */
export const remotePath = (url: string, base: string): string | null => {
  const prefix = `${base}${REMOTE_DIR}`;
  if (url.startsWith(prefix)) return url.slice(prefix.length);
  const match = LOOPBACK.exec(url);
  return match ? (match[1] ?? null) : null;
};
