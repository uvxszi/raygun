// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From lib/intercept.ts: what is taken out of YouTube's own answers before the page sees them.

import {
  blocklistChannelsMatch,
  blocklistTextsMatch,
  isBlocklistEmpty,
  type Blocklist,
} from './blocklist';
import { isObject, pick, pickArray, textOf, type JsonObject } from './json';

/** The player's ad schedule: without it the player never goes after the ads. */
const AD_KEYS = ['adBreakHeartbeatParams', 'adPlacements', 'adSlots', 'playerAds'];

/** YouTube's own API calls whose answers are filtered, by the endpoint's name. */
export const RE_INTERCEPT = /^\/youtubei\/v1\/(browse|get_watch|next|player|reel|search)/;

export type TransformOptions = {
  hideShorts?: boolean;
  showOriginalTitles?: boolean;
  // A choice of its own: with it off the rest still applies and ads reach the page, which
  // supports the people who make the videos.
  blockAds?: boolean;
};

const blocksAds = (options: TransformOptions) => options.blockAds !== false;

const stripAdKeys = (data: unknown) => {
  if (!isObject(data)) return;
  for (const key of AD_KEYS) delete data[key];
};

/* A player answer wherever it comes from: a request the page made, or the one the watch page
 * embeds for its first video (see ../intercept.ts). Changed in place. */
export function filterPlayerResponse(data: unknown, options: TransformOptions = {}) {
  if (blocksAds(options)) stripAdKeys(data);
  applyPlayerResponseOriginalTitle(data, options);
  rewriteOriginalTitles(data, options);
  return data;
}

export function transformPlayerResponse(
  text: string,
  _blocklist?: Blocklist,
  options: TransformOptions = {},
) {
  return JSON.stringify(filterPlayerResponse(JSON.parse(text), options));
}

/** `get_watch`: the player answer and the watch page's own, in one list. */
export function transformGetWatchResponse(
  text: string,
  blocklist?: Blocklist,
  options: TransformOptions = {},
) {
  const data: unknown = JSON.parse(text);
  for (const part of Array.isArray(data) ? data : [data]) {
    const playerResponse = pick(part, 'playerResponse');
    if (blocksAds(options)) stripAdKeys(playerResponse);
    applyPlayerResponseOriginalTitle(playerResponse, options);
  }
  rewriteOriginalTitles(data, options);
  // The watch page's half lists the videos beside the player, ads and Shorts among them.
  filterListResponse(data, blocklist, options);
  return JSON.stringify(data);
}

export function transformSearchResponse(
  text: string,
  blocklist?: Blocklist,
  options: TransformOptions = {},
) {
  const data: unknown = JSON.parse(text);
  const hideShorts = options.hideShorts !== false;
  rewriteOriginalTitles(data, options);
  const sectionListRenderer =
    // mobile
    pick(data, 'contents', 'sectionListRenderer') ||
    // desktop
    pick(
      data,
      'contents',
      'twoColumnSearchResultsRenderer',
      'primaryContents',
      'sectionListRenderer',
    );
  const itemSectionRenderer =
    pick(sectionListRenderer, 'contents', 0, 'itemSectionRenderer') ||
    pick(
      data,
      'onResponseReceivedCommands',
      0,
      'appendContinuationItemsAction',
      'continuationItems',
      0,
      'itemSectionRenderer',
    );
  if (isObject(itemSectionRenderer) && Array.isArray(itemSectionRenderer.contents)) {
    itemSectionRenderer.contents = itemSectionRenderer.contents.filter(
      (item: unknown) =>
        !(hideShorts && isShortsItem(item)) && !itemMatchesBlocklist(item, blocklist),
    );
  }
  filterListResponse(data, blocklist, { ...options, hideShorts });
  return JSON.stringify(data);
}

export function transformBrowseResponse(
  text: string,
  blocklist?: Blocklist,
  options: TransformOptions = {},
) {
  const data: unknown = JSON.parse(text);
  rewriteOriginalTitles(data, options);
  filterListResponse(data, blocklist, options);
  return JSON.stringify(data);
}

// The Shorts feed: reel/reel_watch_sequence lists the upcoming shorts (ads are entries of their
// own), reel/reel_item_watch describes a single one.
export function transformReelResponse(
  text: string,
  blocklist?: Blocklist,
  options: TransformOptions = {},
) {
  const data: unknown = JSON.parse(text);
  if (blocksAds(options)) stripAdKeys(pick(data, 'playerResponse'));
  rewriteOriginalTitles(data, options);
  filterListResponse(data, blocklist, options);
  return JSON.stringify(data);
}

/** Drops ads, Shorts (when asked) and hidden channels' videos from every list in `data`, in place. */
export function filterListResponse(
  data: unknown,
  blocklist?: Blocklist,
  options: TransformOptions = {},
) {
  if (!isObject(data)) return;

  const dropAds = blocksAds(options);
  for (const key of Object.keys(data)) {
    const value = data[key];
    if (Array.isArray(value)) {
      const kept = value.filter(
        (item: unknown) =>
          !(dropAds && isAdItem(item)) &&
          !(options.hideShorts && isShortsItem(item)) &&
          !itemMatchesBlocklist(item, blocklist),
      );
      data[key] = kept;
      kept.forEach((item: unknown) => filterListResponse(item, blocklist, options));
    } else if (isObject(value)) {
      filterListResponse(value, blocklist, options);
    }
  }
}

const SHORTS_PATH = /^\/shorts(?:\/|\?|$)/;

// Only the card itself and known card wrappers are looked at. A search through the whole card
// would also take ordinary videos with a Shorts link in their menu or metadata.
function isShortsItem(item: unknown): boolean {
  if (!isObject(item)) return false;
  const candidates = [
    item,
    pick(item, 'richItemRenderer', 'content'),
    pick(item, 'richSectionRenderer', 'content'),
  ].filter(isObject);
  return candidates.some((card) => {
    if (
      card.reelShelfRenderer ||
      card.reelItemRenderer ||
      card.shortsLockupViewModel ||
      card.shortsShelfViewModel
    ) {
      return true;
    }
    if (pick(card, 'richShelfRenderer', 'isShorts')) return true;
    if (
      pickArray(card, 'gridShelfViewModel', 'contents').some((child) =>
        Boolean(pick(child, 'shortsLockupViewModel')),
      )
    ) {
      return true;
    }
    const renderer =
      card.videoWithContextRenderer ||
      card.videoRenderer ||
      card.compactVideoRenderer ||
      card.gridVideoRenderer ||
      card.lockupViewModel;
    const endpoint =
      pick(renderer, 'navigationEndpoint') ||
      pick(renderer, 'onTap', 'innertubeCommand') ||
      pick(renderer, 'rendererContext', 'commandContext', 'onTap', 'innertubeCommand');
    const url = pick(endpoint, 'commandMetadata', 'webCommandMetadata', 'url');
    return Boolean(
      pick(endpoint, 'reelWatchEndpoint') || (typeof url === 'string' && SHORTS_PATH.test(url)),
    );
  });
}

// Renderers that carry an ad or a promotion instead of a video. Dropping the whole list item
// (rather than hiding it with CSS) also takes its grid cell, which would leave a blank tile.
const AD_RENDERER_KEYS = [
  'adSlotRenderer',
  'adsEngagementPanelRenderer',
  'bannerPromoRenderer',
  'brandVideoShelfRenderer',
  'brandVideoSingletonRenderer',
  'carouselAdRenderer',
  'compactPromotedItemRenderer',
  'compactPromotedVideoRenderer',
  'displayAdRenderer',
  'inFeedAdLayoutRenderer',
  'mealbarPromoRenderer',
  'primetimePromoRenderer',
  'promotedSparklesTextSearchRenderer',
  'promotedSparklesWebRenderer',
  'promotedVideoRenderer',
  'searchPyvRenderer',
  'statementBannerRenderer',
];

function isAdItem(item: unknown): boolean {
  if (!isObject(item)) return false;

  // An ad in the Shorts feed is an ordinary reel entry marked as an ad.
  if (pick(item, 'command', 'reelWatchEndpoint', 'adClientParams', 'isAd')) return true;

  // Wrappers keep the ad a level down, e.g. richItemRenderer.content.adSlotRenderer.
  const sectionContents = pickArray(item, 'itemSectionRenderer', 'contents');
  const candidates = [
    item,
    pick(item, 'richItemRenderer', 'content'),
    pick(item, 'richSectionRenderer', 'content'),
    sectionContents.length === 1 ? sectionContents[0] : undefined,
  ].filter(isObject);

  return candidates.some((candidate) => AD_RENDERER_KEYS.some((key) => candidate[key]));
}

function itemMatchesBlocklist(item: unknown, blocklist?: Blocklist) {
  if (!blocklist || isBlocklistEmpty(blocklist)) return false;
  return getDirectRenderers(item).some((renderer) => rendererMatchesBlocklist(renderer, blocklist));
}

function getDirectRenderers(item: unknown): JsonObject[] {
  if (!isObject(item)) return [];
  return [
    item.videoRenderer,
    item.compactVideoRenderer,
    item.gridVideoRenderer,
    item.videoWithContextRenderer,
    item.reelItemRenderer,
    item.lockupViewModel,
    pick(item, 'richItemRenderer', 'content', 'videoRenderer'),
    pick(item, 'richItemRenderer', 'content', 'lockupViewModel'),
  ].filter(isObject);
}

function rendererMatchesBlocklist(renderer: JsonObject, blocklist: Blocklist) {
  return (
    blocklistTextsMatch(extractTitleTexts(renderer), blocklist.keywords) ||
    blocklistChannelsMatch(extractChannelTexts(renderer), blocklist.channels)
  );
}

function extractTitleTexts(renderer: JsonObject): string[] {
  return [
    textOf(renderer.title),
    textOf(renderer.headline),
    textOf(pick(renderer, 'metadata', 'lockupMetadataViewModel', 'title')),
  ].filter(Boolean);
}

type BrowseEndpoints = { ids: Set<string>; urls: Set<string>; names: Set<string> };

/** Every channel a card links to: its id, its address and the linked name. */
function findBrowseEndpoints(
  node: unknown,
  results: BrowseEndpoints = { ids: new Set(), urls: new Set(), names: new Set() },
) {
  if (!isObject(node)) return results;

  const browseEndpoint = node.browseEndpoint;
  if (isObject(browseEndpoint)) {
    const { browseId, canonicalBaseUrl } = browseEndpoint;
    if (typeof browseId === 'string' && browseId) results.ids.add(browseId);
    if (typeof canonicalBaseUrl === 'string' && canonicalBaseUrl)
      results.urls.add(canonicalBaseUrl);
  }

  if (
    typeof node.text === 'string' &&
    node.text &&
    pick(node, 'navigationEndpoint', 'browseEndpoint')
  ) {
    results.names.add(node.text);
  }

  for (const key of Object.keys(node)) findBrowseEndpoints(node[key], results);
  return results;
}

function extractChannelTexts(renderer: JsonObject): string[] {
  const endpoints = findBrowseEndpoints(renderer);
  const texts = [...endpoints.names, ...endpoints.urls, ...endpoints.ids];
  if (texts.length) return texts;

  const run = (field: string, key: string) =>
    pick(renderer, field, 'runs', 0, 'navigationEndpoint', 'browseEndpoint', key);
  return [
    textOf(renderer.ownerText),
    textOf(renderer.shortBylineText),
    textOf(renderer.longBylineText),
    textOf(pick(renderer, 'owner', 'videoOwnerRenderer', 'title')),
    run('shortBylineText', 'canonicalBaseUrl'),
    run('shortBylineText', 'browseId'),
    run('ownerText', 'canonicalBaseUrl'),
    run('ownerText', 'browseId'),
    textOf(pick(renderer, 'metadata', 'lockupMetadataViewModel', 'metadata')),
    pickArray(renderer, 'contentMetadata', 'runs')
      .map((part) => textOf(pick(part, 'text')))
      .join(' '),
  ].filter((text): text is string => typeof text === 'string' && text !== '');
}

function applyPlayerResponseOriginalTitle(data: unknown, options: TransformOptions) {
  if (!options.showOriginalTitles) return;

  // videoDetails keeps the title as uploaded; the rest of the answer carries the translation.
  const originalTitle = textOf(pick(data, 'videoDetails', 'title'));
  if (!originalTitle) return;

  setTextNode(pick(data, 'microformat', 'playerMicroformatRenderer', 'title'), originalTitle);
  setTextNode(pick(data, 'videoDetails', 'title'), originalTitle);
}

function rewriteOriginalTitles(data: unknown, options: TransformOptions) {
  if (!options.showOriginalTitles || !isObject(data)) return;

  walk(data, (node) => {
    for (const renderer of getDirectRenderers(node)) rewriteRendererTitle(renderer);
    rewriteRendererTitle(node);
  });
}

function walk(node: unknown, visit: (node: JsonObject) => void) {
  if (!isObject(node)) return;
  visit(node);
  for (const key of Object.keys(node)) walk(node[key], visit);
}

function rewriteRendererTitle(renderer: JsonObject) {
  const originalTitle = getOriginalTitle(renderer);
  if (!originalTitle) return;

  setTextNode(renderer.title, originalTitle);
  setTextNode(renderer.headline, originalTitle);
  setTextNode(pick(renderer, 'metadata', 'lockupMetadataViewModel', 'title'), originalTitle);
}

function getOriginalTitle(renderer: JsonObject): string {
  return (
    [
      renderer.originalTitle,
      renderer.untranslatedTitle,
      renderer.untranslatedVideoTitle,
      pick(renderer, 'title', 'originalTitle'),
      pick(renderer, 'title', 'untranslatedTitle'),
      pick(renderer, 'headline', 'originalTitle'),
      pick(renderer, 'metadata', 'lockupMetadataViewModel', 'originalTitle'),
      pick(renderer, 'metadata', 'lockupMetadataViewModel', 'title', 'originalTitle'),
    ]
      .map(textOf)
      .find(Boolean) || ''
  );
}

function setTextNode(node: unknown, text: string) {
  if (!isObject(node)) return;

  if (typeof node.simpleText === 'string') node.simpleText = text;
  if (typeof node.content === 'string') node.content = text;
  if (Array.isArray(node.runs) && node.runs.length) {
    node.runs.forEach((run: unknown, index: number) => {
      if (isObject(run) && typeof run.text === 'string') run.text = index === 0 ? text : '';
    });
  }
}
