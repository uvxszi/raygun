// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From lib/user-styles.ts: its built-in styles, each switched by a setting of this extension.

/** Styles that take parts of YouTube away, by the setting that turns each on. */
export const builtinStyles: { setting: string; css: string }[] = [
  {
    setting: 'hideMixes',
    // Every playlist thumbnail has a collections stack now, so the mix link is what's matched,
    // or a channel's Playlists tab would end up empty.
    css: `
      ytm-compact-radio-renderer,
      ytm-compact-playlist-renderer:has(a[href*='start_radio=1']),
      ytm-rich-item-renderer:has(a[href*='start_radio=1']),
      ytd-radio-renderer,
      ytd-compact-radio-renderer,
      ytd-rich-item-renderer:has(a[href*='start_radio=1']) {
        display: none !important;
      }
    `,
  },
  {
    setting: 'hideCommunityPosts',
    css: `
      ytd-rich-section-renderer:has(ytd-post-renderer),
      ytd-post-renderer,
      ytd-backstage-post-thread-renderer,
      ytm-rich-section-renderer:has(ytm-post-renderer),
      ytm-post-renderer,
      ytm-backstage-post-thread-renderer {
        display: none !important;
      }
    `,
  },
  {
    setting: 'hideAskButton',
    css: `
      .you-chat-entrypoint-button,
      yt-button-view-model:has(> .you-chat-entrypoint-button) {
        display: none !important;
      }
    `,
  },
  {
    setting: 'hideHomeFeed',
    // m.youtube.com has no page-subtype attribute; its home tab is told by its browse endpoint
    // (FEwhat_to_watch) on the tab's wrapper.
    css: `
      ytd-browse[page-subtype='home'] #contents,
      ytm-browse [tab-identifier='FEwhat_to_watch'] ytm-rich-grid-renderer {
        display: none !important;
      }
    `,
  },
  {
    setting: 'hideRelatedVideos',
    css: `
      #related,
      ytd-watch-next-secondary-results-renderer,
      ytm-watch-next-secondary-results-renderer,
      ytm-item-section-renderer[section-identifier='related-items'] {
        display: none !important;
      }
    `,
  },
  {
    setting: 'hideEndScreens',
    css: `
      .ytp-ce-element,
      .ytp-endscreen-content,
      .ytp-suggested-video-overlay,
      .ytp-pause-overlay,
      .ytp-pause-overlay-container {
        display: none !important;
      }
    `,
  },
];

/** The Shorts tab and its links in the navigation, hidden with the Shorts in the feeds. */
export const shortsNavigationCss = `
  yt-tab-shape[tab-title='Shorts'],
  ytd-guide-entry-renderer:has(a[href^='/shorts']),
  ytd-mini-guide-entry-renderer:has(a[href^='/shorts']),
  ytm-pivot-bar-item-renderer:has(.pivot-shorts) {
    display: none !important;
  }
`;
