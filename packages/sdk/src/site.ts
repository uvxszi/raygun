// Site scripts (`site.scripts`): scripts and styles that Raygun's built-in browser adds to the
// pages of the sites an extension names. They run in the page, with the page's privileges only:
// nothing of Raygun (its own webview, files, other sites) is reachable from them. What they can
// hand to Raygun is data, through `raygunPage`, which Raygun checks before it acts on it.
// The guide is docs/extensions/site-scripts.md.

/** A value of one of the extension's settings: text and choices are strings. */
export type SiteSettingValue = string | number | boolean | null;

/** What a page offers to Raygun: shown as actions in the browser (Play in Raygun, Download...). */
export type PageOffer = {
  /**
   * `video`: something to watch; `song`: something to listen to; `playlist`: a list of either,
   * opened in Raygun's search.
   */
  kind: 'video' | 'song' | 'playlist';
  /** Its own address on the site, http(s). Offers with the same address replace each other. */
  url: string;
  title?: string;
  /** Who made it: the artist, or a video's channel. */
  artist?: string;
  album?: string;
  /** A picture of it, http(s). */
  artworkUrl?: string;
  durationMs?: number;
};

/** What the browser says about where the page is. */
export type PageState = {
  /** The browser is off screen: Raygun is in the background, the screen is off, the window is minimized. */
  background: boolean;
  /** A phone call is on (Android). */
  call: boolean;
  /** Something plays sound on the device, this page included (Android). */
  audioActive: boolean;
};

export type RaygunPage = {
  /** The extension's id. */
  readonly extension: string;
  /** Its version. */
  readonly version: string;
  /** `android` or `desktop`. */
  readonly platform: 'android' | 'desktop';
  /** The extension's settings as the user set them (else their defaults), by key. */
  readonly settings: Readonly<Record<string, SiteSettingValue>>;
  /** What the browser says now; see `on('state')` for changes. */
  readonly state: Readonly<PageState>;
  /**
   * Offers something on the page to Raygun. The browser shows the latest offers of the page
   * shown, with what Raygun can do with each; nothing happens until someone picks an action.
   */
  offer(offer: PageOffer): void;
  /** Takes back the offer with this address, or all of this page's offers. */
  withdraw(url?: string): void;
  /** Calls `listener` whenever the browser's state changes; returns a function that stops it. */
  on(event: 'state', listener: (state: Readonly<PageState>) => void): () => void;
};

declare global {
  /** Raygun's page API, given to each of a site-scripts extension's scripts. */
  const raygunPage: RaygunPage;
}
