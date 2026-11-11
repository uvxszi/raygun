// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/intercept.ts: YouTube's answers are filtered (./lib/intercept.ts) on their way
// into the page, whether they came with it or were fetched later.

import type { Blocklist } from './lib/blocklist';
import {
  RE_INTERCEPT,
  filterListResponse,
  filterPlayerResponse,
  transformBrowseResponse,
  transformGetWatchResponse,
  transformPlayerResponse,
  transformReelResponse,
  transformSearchResponse,
  type TransformOptions,
} from './lib/intercept';

type Transform = (
  text: string,
  blocklist: Blocklist | undefined,
  options: TransformOptions,
) => string;

const TRANSFORMS: Record<string, Transform> = {
  browse: transformBrowseResponse,
  get_watch: transformGetWatchResponse,
  next: transformBrowseResponse,
  player: transformPlayerResponse,
  reel: transformReelResponse,
  search: transformSearchResponse,
};

const report = (where: string, error: unknown) =>
  console.error(`[YouTube enhancements] ${where}:`, error);

/** The endpoint an address calls (`browse`, `player`…), when it is one of those filtered. */
const endpointOf = (address: string): string | undefined => {
  try {
    return new URL(address, location.origin).pathname.match(RE_INTERCEPT)?.[1];
  } catch {
    return undefined;
  }
};

/** A global the page sets once (in an inline script), filtered as it is set. */
function filterGlobal(name: string, filter: (value: unknown) => unknown) {
  const run = (value: unknown) => {
    try {
      return filter(value);
    } catch (error) {
      report(name, error);
      return value;
    }
  };
  // Filtered once more in case this script landed after the page had already set it.
  let current = run((window as unknown as Record<string, unknown>)[name]);
  Object.defineProperty(window, name, {
    get: () => current,
    set: (value: unknown) => {
      current = run(value);
    },
    configurable: true,
  });
}

export function installIntercept(options: TransformOptions, blocklist: Blocklist | undefined) {
  // The first page's data comes in the page itself, in script tags.
  filterGlobal('ytInitialData', (value) => {
    filterListResponse(value, blocklist, options);
    return value;
  });

  // The watch page embeds its first player answer as well. Left alone, its ad placements reach
  // the player, which then goes after ads the browser blocks.
  filterGlobal('ytInitialPlayerResponse', (value) => filterPlayerResponse(value, options));

  // The first batch of the Shorts feed is in the page too, as JSON text the page parses itself.
  filterGlobal('ytInitialReelWatchSequenceResponse', (value) => {
    const reelOptions = { blockAds: options.blockAds };
    if (typeof value === 'string') return transformReelResponse(value, blocklist, reelOptions);
    filterListResponse(value, blocklist, reelOptions);
    return value;
  });

  const pageFetch = window.fetch;
  window.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const response = await pageFetch.call(window, input, init);
    const address = input instanceof Request ? input.url : input.toString();
    const transform = TRANSFORMS[endpointOf(address) ?? ''];
    if (!transform || response.status !== 200) return response;

    const text = await response.text();
    const responseInit = {
      status: response.status,
      statusText: response.statusText,
      headers: response.headers,
    };
    try {
      return new Response(transform(text, blocklist, options), responseInit);
    } catch (error) {
      report('fetch', error);
    }
    return new Response(text, responseInit);
  };

  // Filtered when first read, not in a readystatechange listener: the page's own listener may
  // have been added before open(), and JSON requests have no responseText.
  const responseGetter = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, 'response')?.get;
  const textGetter = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, 'responseText')?.get;
  if (!responseGetter || !textGetter) return;
  const xhrOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (
    this: XMLHttpRequest,
    method: string,
    url: string | URL,
    ...rest: unknown[]
  ) {
    const transform = TRANSFORMS[endpointOf(url.toString()) ?? ''];
    let cached = false;
    let filteredText: unknown;
    let filteredJson: unknown;
    const read = (asText: boolean): unknown => {
      const original = () => (asText ? textGetter : responseGetter).call(this);
      if (
        !transform ||
        this.readyState !== 4 ||
        this.status !== 200 ||
        (this.responseType !== '' && this.responseType !== 'text' && this.responseType !== 'json')
      ) {
        return original();
      }
      // responseText of a JSON request throws InvalidStateError, and keeps doing so.
      if (asText && this.responseType === 'json') return original();
      const isJson = this.responseType === 'json';
      if (cached) return isJson ? filteredJson : filteredText;
      cached = true;
      try {
        const text = isJson ? JSON.stringify(responseGetter.call(this)) : textGetter.call(this);
        filteredText = transform(String(text), blocklist, options);
        if (isJson) filteredJson = JSON.parse(String(filteredText));
      } catch (error) {
        report('XMLHttpRequest', error);
        // A changed or broken answer passes through as it is, once, rather than being parsed
        // and reported again on every read.
        if (isJson) filteredJson = responseGetter.call(this);
        else filteredText = textGetter.call(this);
      }
      return isJson ? filteredJson : filteredText;
    };
    Object.defineProperties(this, {
      response: { configurable: true, get: () => read(false) },
      responseText: { configurable: true, get: () => read(true) },
    });
    // The async flag and the credentials are passed on as given, synchronous requests included.
    return Reflect.apply(xhrOpen, this, [method, url, ...rest]);
  };
}
