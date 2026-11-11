// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/css.ts, with the built-in styles of lib/user-styles.ts: the extension's one style
// sheet, made of the parts its settings turn on. Parts for NouTube's own controls are left out.

import { builtinStyles, shortsNavigationCss } from './lib/user-styles';
import { addStyle, flag, isMobileAgent, isMobileSite, isMusic } from './page';

// Only while ad blocking is on; with it off the ads YouTube serves stay visible.
const adsCss = `
  ytd-page-top-ad-layout-renderer,
  ytd-in-feed-ad-layout-renderer,
  ad-slot-renderer,
  yt-mealbar-promo-renderer,
  ytm-promoted-sparkles-web-renderer,
  .ytd-player-legacy-desktop-watch-ads-renderer {
    display: none !important;
  }

  /* Ads are normally dropped from the feed's data, but if one still shows, its grid cell goes
   * too, or the hidden ad would leave a blank tile behind. */
  ytd-rich-item-renderer:has(ytd-ad-slot-renderer),
  ytd-rich-item-renderer:has(ytd-in-feed-ad-layout-renderer),
  ytd-rich-section-renderer:has(ytd-ad-slot-renderer),
  ytd-rich-section-renderer:has(ytd-statement-banner-renderer),
  ytd-item-section-renderer:has(ytd-ad-slot-renderer),
  ytm-rich-item-renderer:has(ad-slot-renderer),
  ytm-item-section-renderer:has(ad-slot-renderer),
  ytd-rich-item-renderer:has(ad-slot-renderer),
  ytd-rich-item-renderer:has(.ytwFeedAdMetadataViewModelHostMetadata) {
    display: none !important;
  }
`;

// The mobile sites' "Open App" links, which only lead out of the browser. NouTube hides every
// link-shaped button; here only links that open an app (Android intents) go, so links such as
// "Sign in" stay.
const appLinksCss = `
  a.app-install-link,
  a[href^='intent:'],
  a[href^='vnd.youtube:'] {
    display: none !important;
  }
`;

const shortsCss = `
  ytm-reel-shelf-renderer,
  ytd-reel-shelf-renderer,
  .ytGridShelfViewModelHost,
  grid-shelf-view-model,
  ytd-rich-section-renderer:has(ytd-rich-shelf-renderer[is-shorts]),
  ytm-shorts-lockup-view-model,
  yt-lockup-view-model:has(a[href^='/shorts']),
  ytd-video-renderer:has(a[href^='/shorts']),
  ytm-video-with-context-renderer:has(a[href^='/shorts']) {
    display: none !important;
  }
`;

const sponsorSegmentsCss = `
  ._nou_sb_segments {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 1;
  }
  ._nou_sb_segment {
    position: absolute;
    top: 0;
    height: 100%;
    opacity: 0.85;
  }
`;

// The mobile player's controls fade to fully transparent at the bottom, where the progress bar
// sits, so its track (white at 35%) disappears over a bright frame. Darkening the bottom and
// outlining the bar keeps it and the sponsored segments on it readable.
const mobileProgressCss = `
  #player-control-overlay .player-controls-background::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.7), rgba(0, 0, 0, 0) 35%);
    pointer-events: none;
  }

  #player-control-overlay .ytPlayerProgressBarHost {
    filter: drop-shadow(0 1px 1px rgba(0, 0, 0, 0.6));
  }
`;

// YouTube Music on phones.
const musicMobileCss = `
  /* Minimized to the player bar, a typo in YouTube Music's own collapse selector leaves the
   * layout fixed and the full player's backgrounds and overlays over what is browsed. */
  ytmusic-app-layout[player-ui-state='PLAYER_BAR_ONLY'] {
    position: static !important;
    animation: none !important;
    transform: none !important;
  }

  ytmusic-player-page[player-ui-state='PLAYER_BAR_ONLY'] {
    visibility: hidden !important;
    background: transparent !important;
    pointer-events: none !important;
  }

  ytmusic-player-page[player-ui-state='PLAYER_BAR_ONLY'] ytmusic-player {
    visibility: visible !important;
    pointer-events: auto !important;
  }

  /* Full screen sizes the player to 56.25vw (16:9 of the width), which runs off the screen on
   * phones wider than 16:9. */
  ytmusic-player[player-ui-state='FULLSCREEN'][is-mweb-modernization-enabled][video-mode] {
    height: min(56.25vw, 100vh) !important;
  }
`;

const liveChatCss = `
  #_nou_livechat {
    width: 100%;
    height: 50vh;
    position: fixed;
    bottom: 0;
    display: flex;
    flex-direction: column;
    border-top: 1px solid #e5e5e5;
    background: white;
    z-index: 10;
  }
  #_nou_livechat.right {
    width: 36vw;
    height: 100%;
    top: 0;
    bottom: 0;
    right: 0;
    border-top: none;
    border-left: 1px solid #e5e5e5;
  }

  #_nou_livechat > button {
    position: absolute;
    top: 1.25rem;
    left: 50%;
    transform: translateX(-50%);
  }

  #_nou_livechat_loading {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
  }

  #_nou_livechat iframe {
    position: relative;
    flex: 1;
    border: none;
  }

  ._nou_replay_panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    color: #0f0f0f;
    font-size: 14px;
  }
  ._nou_replay_panel header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
    min-height: 56px;
    box-sizing: border-box;
    padding: 6px 12px 6px 16px;
    border-bottom: 1px solid #e5e5e5;
    font-size: 16px;
    font-weight: 600;
  }
  ._nou_replay_panel header ._nou_livechat_close {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  ._nou_replay_panel header ._nou_livechat_close:is(:hover, :active) {
    background: #f2f2f2;
  }
  ._nou_replay_panel header ._nou_livechat_close:focus-visible {
    outline: 2px solid #606060;
    outline-offset: -4px;
  }
  ._nou_replay_panel header ._nou_livechat_close svg {
    width: 20px;
    height: 20px;
  }
  ._nou_replay_panel p {
    padding: 0.5rem 1rem;
  }
  ._nou_replay_messages {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    padding: 0.5rem 1rem;
  }
  ._nou_replay_message {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 6px 0;
    overflow-wrap: anywhere;
  }
  ._nou_replay_message small,
  ._nou_replay_message strong {
    color: #606060;
    margin-right: 6px;
  }
  ._nou_replay_avatar {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  ._nou_replay_emoji {
    width: 20px;
    height: 20px;
    vertical-align: middle;
  }
  ._nou_replay_sticker {
    display: block;
    width: 64px;
    height: 64px;
    object-fit: contain;
    margin-top: 4px;
  }
  ._nou_replay_retry {
    align-self: flex-start;
    margin: 0 1rem 0.5rem;
    padding: 0.5rem 1rem;
  }

  #_nou_livechat_btn {
    padding: 0.75rem 1rem;
    background: #e1002d;
    color: white;
    position: fixed;
    bottom: 1rem;
    right: 1rem;
    border-radius: 18px;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 16px;
  }
`;

/** The style sheet for this page, as the settings have it. */
export function buildCss(): string {
  const parts: string[] = [];
  if (flag('blockAds', true)) parts.push(adsCss, appLinksCss);
  if (flag('hideShorts', true) && !isMusic) parts.push(shortsCss, shortsNavigationCss);
  if (flag('sponsorBlock', true)) {
    parts.push(sponsorSegmentsCss);
    if (isMobileSite) parts.push(mobileProgressCss);
  }
  if (isMusic && isMobileAgent) parts.push(musicMobileCss);
  if (isMobileSite && flag('liveChat', true)) parts.push(liveChatCss);
  if (!isMusic) {
    for (const style of builtinStyles) {
      if (flag(style.setting, false)) parts.push(style.css);
    }
  }
  return parts.join('\n');
}

export function injectCss() {
  const css = buildCss();
  if (css) addStyle('_nou_injected_css', css);
}
