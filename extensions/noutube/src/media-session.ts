// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/background-guard.ts (guardMediaSessionHandlers).
//
// The media notification's buttons, a headset's and the lock screen's reach the page through
// the handlers YouTube registers with the Media Session. Wrapping the registration is the only
// way to see those presses: the background guard tells them from YouTube's own pauses, and on
// the mobile site next and previous are kept inside a playlist.
//
// YouTube registers its handlers while its first scripts run, so this has to be in place at
// document start, before any of them.

declare global {
  interface MediaSession {
    __nouWrapped?: boolean;
  }
}

/** Sees a press before YouTube's handler does; true when it took the press itself. */
type MediaActionHook = (details: MediaSessionActionDetails) => boolean;

const hooks = new Map<MediaSessionAction, MediaActionHook[]>();

// The wrapped session, held: a browser may collect the script object for
// navigator.mediaSession while nothing holds it, and the wrapper would go with it.
let wrapped: MediaSession | undefined;

/** Lets `hook` see presses of `action` first, from now on. */
export function hookMediaAction(action: MediaSessionAction, hook: MediaActionHook) {
  hooks.set(action, [...(hooks.get(action) ?? []), hook]);
}

export function wrapMediaSession() {
  const session = navigator.mediaSession;
  if (!session?.setActionHandler || session.__nouWrapped) return;
  const original = session.setActionHandler.bind(session);
  session.setActionHandler = (action, handler) => {
    if (!handler) {
      original(action, handler);
      return;
    }
    original(action, (details) => {
      for (const hook of hooks.get(action) ?? []) {
        try {
          if (hook(details)) return;
        } catch (error) {
          // A hook that fails leaves the press to YouTube.
          console.error('[YouTube enhancements] media controls:', error);
        }
      }
      handler(details);
    });
  };
  session.__nouWrapped = true;
  wrapped = session;
}

/** Whether presses reach the hooks (the session could be wrapped). */
export const mediaSessionWrapped = () => wrapped !== undefined;
