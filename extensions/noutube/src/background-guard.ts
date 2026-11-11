// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/background-guard.ts, and the _lact refresher of content/main.ts.
//
// YouTube stops background playback on purpose: the mobile site pauses for those without
// Premium once it believes the page is hidden, and inactivity prompts ("Video paused. Continue
// watching?", ytmusic-you-there-renderer) pause after a few hours. Raygun's browser keeps
// telling the page it is shown, so the first rarely happens; it does say (raygunPage.state)
// when it is really off screen.
//
// Off screen, people can't reach the page, so a pause they didn't ask for -- through the media
// notification, a headset or the lock screen, all of which go through the page's Media Session
// handlers (see ./media-session.ts) -- and that isn't an audio interruption (a call, another
// app playing) can only come from YouTube itself: the prompt, if one is shown, is answered and
// the video plays on.

import { hookMediaAction, mediaSessionWrapped } from './media-session';
import { pageState } from './page';
import { PlayerState, getPlayer } from './player';

let userPaused = false;
let wasPlaying = false;
let pausedByInterruption = false;
let pausedTicks = 0;
let resumeAttempts = 0;

const isBackground = () => pageState().background;

// A call, or anything else playing sound: NouTube's canAutoResume.
const canAutoResume = () => {
  const state = pageState();
  return !state.call && !state.audioActive;
};

function confirmYouThereDialogs() {
  // "Continue watching?" prompts. Only a dialog with a single button is clicked, so a wrong
  // guess is impossible; with more buttons playVideo() below plays just the same and YouTube
  // drops the prompt itself.
  for (const selector of ['ytmusic-you-there-renderer', 'ytm-confirm-dialog-renderer']) {
    const dialog = document.querySelector(selector);
    if (!dialog) continue;
    const buttons = dialog.querySelectorAll('button');
    const [only] = buttons;
    if (buttons.length === 1 && only) {
      only.click();
      return true;
    }
    // A dialog left in the page but not clickable must not hide the next one.
  }
  return false;
}

/** Marks presses of the system's media controls as the listener's own, at document start. */
export function guardMediaSessionHandlers() {
  // Asked for through the system's controls, so the pause is theirs however long it lasts.
  hookMediaAction('pause', () => {
    userPaused = true;
    return false;
  });
  hookMediaAction('play', () => {
    userPaused = false;
    return false;
  });
  hookMediaAction('stop', () => {
    userPaused = true;
    return false;
  });
}

const inPlayer = (event: Event) => {
  const player = getPlayer();
  return event.target instanceof Node && Boolean(player?.contains(event.target));
};

export function installBackgroundGuard() {
  // Without the Media Session wrapped, a pause from the notification can't be told from
  // YouTube's own, and the guard would undo it.
  if (!mediaSessionWrapped()) return;

  // A pause from YouTube's own controls, made while the page is on screen, is recorded at once,
  // so leaving the screen right after can't race the check below into playing it again.
  document.addEventListener(
    'pause',
    (event) => {
      if (!isBackground() && inPlayer(event)) userPaused = true;
    },
    true,
  );

  // Playback that starts just before the browser leaves the screen would otherwise not be seen
  // by the check below, and a pause by YouTube then would look like a video that never played.
  for (const type of ['play', 'playing']) {
    document.addEventListener(
      type,
      (event) => {
        if (!inPlayer(event)) return;
        userPaused = false;
        wasPlaying = true;
      },
      true,
    );
  }

  setInterval(() => {
    const player = getPlayer();
    if (!player?.getPlayerState) return;

    const state = player.getPlayerState();
    if (state === PlayerState.playing) {
      userPaused = false;
      wasPlaying = true;
      pausedByInterruption = false;
      pausedTicks = 0;
      resumeAttempts = 0;
      return;
    }

    if (!isBackground()) {
      // The page can be reached again; whatever is paused now is the listener's business.
      wasPlaying = false;
      pausedByInterruption = false;
      pausedTicks = 0;
      resumeAttempts = 0;
      return;
    }

    if (state !== PlayerState.paused || !wasPlaying || userPaused || pausedByInterruption) return;
    pausedTicks++;
    if (resumeAttempts >= 3) return;
    if (!canAutoResume()) {
      // The page's own sound lingers as "playing" for some seconds after a pause, so it gets a
      // few checks to clear. Still taken after that means a call or another app has the sound:
      // the video stays paused for good, and the notification plays it again when wanted.
      if (pausedTicks >= 6) pausedByInterruption = true;
      return;
    }

    resumeAttempts++;
    confirmYouThereDialogs();
    player.playVideo?.();
  }, 5000);
}

/**
 * YouTube pauses with a "Continue watching?" prompt once its last-activity time (_lact) is
 * hours old. Refreshing it every minute keeps the session active for as long as it plays.
 */
export function keepSessionActive() {
  setInterval(() => {
    window._lact = Date.now();
  }, 60 * 1000);
}
