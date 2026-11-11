// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/blocklist.ts: videos of hidden channels, or with hidden words in their titles,
// are hidden as they are drawn too. Most never get there: ./intercept.ts drops them from
// YouTube's answers.

import {
  blocklistChannelsMatch,
  blocklistTextMatches,
  blocklistTextsMatch,
  type Blocklist,
} from './lib/blocklist';
import { addStyle, whenRoot } from './page';

const blockedClass = '_nou_blocked';

const itemSelector = [
  'ytm-media-item',
  'ytm-compact-video-renderer',
  'ytm-video-with-context-renderer',
  'ytd-video-renderer',
  'ytd-compact-video-renderer',
  'ytd-grid-video-renderer',
  'ytd-rich-item-renderer',
  'yt-lockup-view-model',
  'yt-lockup-metadata-view-model',
].join(',');

const getText = (element: Element | null | undefined) => element?.textContent?.trim() || '';

function getTitleTexts(item: Element) {
  return [
    getText(item.querySelector('h3')),
    getText(item.querySelector('h4')),
    getText(item.querySelector('#video-title')),
    getText(item.querySelector('a#video-title')),
    getText(item.querySelector('.yt-lockup-metadata-view-model-wiz__title')),
    getText(item.querySelector('.yt-core-attributed-string[role="text"]')),
  ].filter(Boolean);
}

function getChannelTexts(item: Element) {
  const channelLinks = Array.from(
    item.querySelectorAll<HTMLAnchorElement>(
      'a[href^="/@"],a[href^="/channel/"],a[href^="/c/"],a[href^="/user/"],a[href*="youtube.com/@"],a[href*="youtube.com/channel/"]',
    ),
  );

  return [
    ...channelLinks.flatMap((link) => [
      link.textContent?.trim(),
      link.getAttribute('href') || '',
      link.href || '',
    ]),
    getText(item.querySelector('.yt-lockup-metadata-view-model-wiz__metadata')),
    getText(item.querySelector('ytm-badge-and-byline-renderer')),
    getText(item.querySelector('ytd-channel-name')),
    getText(item.querySelector('#channel-name')),
  ].filter(Boolean);
}

export function installBlocklistFilter(blocklist: Blocklist) {
  addStyle('_nou_blocklist_css', `.${blockedClass}{display:none!important;}`);

  const itemMatches = (item: Element) =>
    blocklistTextsMatch(getTitleTexts(item), blocklist.keywords) ||
    blocklistChannelsMatch(getChannelTexts(item), blocklist.channels) ||
    blocklistTextMatches(item.getAttribute('aria-label') || '', blocklist.keywords);

  const apply = (item: Element) => item.classList.toggle(blockedClass, itemMatches(item));

  whenRoot((root) => {
    root.querySelectorAll(itemSelector).forEach(apply);
    new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        mutation.addedNodes.forEach((node) => {
          if (!(node instanceof Element)) return;
          if (node.matches(itemSelector)) apply(node);
          const parentItem = node.parentElement?.closest(itemSelector);
          if (parentItem) apply(parentItem);
          node.querySelectorAll(itemSelector).forEach(apply);
        });
      }
    }).observe(root, { childList: true, subtree: true });
  });
}
