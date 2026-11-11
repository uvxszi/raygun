// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/main.ts: what runs on YouTube's pages (www, m and music.youtube.com), each part
// switched by the extension's settings. NouTube's page code talks to its own app; here the
// browser's state, the settings and the offers go through `raygunPage` instead.
//
// The script runs at document start, before any of YouTube's own: the parts that change how the
// page itself works (the Media Session, codecs, thumbnails, YouTube's answers, history) have to
// be in place before YouTube uses them.

import { installOriginalAudio } from './audio';
import { installEncodedAuthorNameFix } from './author-names';
import { installAutoplayPolicy } from './autoplay';
import {
  installBackgroundGuard,
  guardMediaSessionHandlers,
  keepSessionActive,
} from './background-guard';
import { installBlocklistFilter } from './blocklist';
import { installClickbaitThumbnails, thumbnailTarget } from './clickbait';
import { injectCss } from './css';
import { installPromoDismissal } from './dialogs';
import { installDislikeCount } from './dislikes';
import { installH264ify } from './h264ify';
import { installIntercept } from './intercept';
import { blocklistFromSettings, isBlocklistEmpty } from './lib/blocklist';
import { installLiveChat } from './livechat';
import { wrapMediaSession } from './media-session';
import { installOffers } from './offers';
import { flag, isAndroid, isMobileSite, isMusic, textSetting } from './page';
import { watchPlayer } from './player';
import { installPlaylistControls } from './playlist-nav';
import { installSponsorBlock } from './sponsorblock';
import { installWatchNavigation } from './watch-nav';

/** Runs one part; a part that fails leaves the others running. */
function install(part: string, run: () => void) {
  try {
    run();
  } catch (error) {
    console.error(`[YouTube enhancements] ${part}:`, error);
  }
}

function start() {
  const blockAds = flag('blockAds', true);
  const backgroundPlay = flag('backgroundPlay', true);
  const hideShorts = flag('hideShorts', true) && !isMusic;
  const showOriginalTitles = flag('showOriginalTitles', false);
  const blocklist = blocklistFromSettings(
    textSetting('hiddenChannels'),
    textSetting('hiddenWords'),
  );
  const hidesChannels = !isMusic && !isBlocklistEmpty(blocklist);
  // Android's browser plays on with the screen off; the desktop's windows don't pause YouTube.
  const guardBackground = backgroundPlay && isAndroid;

  // Before any of YouTube's scripts: it registers its Media Session handlers early on.
  if (guardBackground || isMobileSite) install('media controls', wrapMediaSession);
  if (guardBackground) install('background play', guardMediaSessionHandlers);
  if (isMobileSite) install('playlist controls', installPlaylistControls);
  if (flag('preferH264', false)) install('H.264', installH264ify);
  const thumbnails = thumbnailTarget(textSetting('thumbnails'));
  if (thumbnails) install('thumbnails', () => installClickbaitThumbnails(thumbnails));
  if (blockAds || hideShorts || showOriginalTitles || hidesChannels) {
    install('filters', () =>
      installIntercept(
        { blockAds, hideShorts, showOriginalTitles },
        hidesChannels ? blocklist : undefined,
      ),
    );
  }
  if (flag('backSkipsWatched', false)) install('history', installWatchNavigation);

  install('styles', injectCss);
  if (backgroundPlay) install('session', keepSessionActive);
  if (guardBackground) install('background play', installBackgroundGuard);
  if (flag('sponsorBlock', true)) install('SponsorBlock', installSponsorBlock);
  if (flag('originalAudio', true)) install('original audio', installOriginalAudio);
  if (flag('dislikes', false) && !isMusic) install('dislikes', installDislikeCount);
  if (flag('liveChat', true) && isMobileSite) install('live chat', installLiveChat);
  if (!isMusic) {
    install('autoplay', () => installAutoplayPolicy(flag('disableAutoplay', false)));
  }
  if (hidesChannels) install('hidden channels', () => installBlocklistFilter(blocklist));
  if (isMobileSite) install('channel names', installEncodedAuthorNameFix);
  if (isMusic && blockAds) install('promotions', installPromoDismissal);
  install('offers', installOffers);
  // Last, so every part above hears about the player from its first video on.
  install('player', watchPlayer);
}

if (!window.__nouRaygun) {
  window.__nouRaygun = true;
  start();
}
