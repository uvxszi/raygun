// Raygun's page bridge for its websites browser, injected into every frame before the page's
// own scripts (Android: `WebViewCompat.addDocumentStartJavaScript`; desktop: an initialization
// script). It carries data only, both ways:
// - what plays in the frame (its media elements and `navigator.mediaSession`), so the browser
//   can show media controls and keep a playing page going while it's off screen;
// - the browser's state (off screen, a call on) and its media controls, back to the page;
// - site scripts' offers (`raygunPage`, composed by `raygun_ext_host::site`), which go through
//   `post` here.
// The page can see and call all of it, as it could do the same itself: nothing here reaches
// Raygun beyond the messages, which the browser and Raygun check.
(function () {
  var HUB = Symbol.for('raygun.page');
  if (window[HUB]) return;
  var top = false;
  try {
    top = window === window.top;
  } catch (error) {
    top = false;
  }
  var android = !!(window.raygunBridge && typeof window.raygunBridge.postMessage === 'function');
  var desktop =
    !android &&
    top &&
    !!(
      window.chrome &&
      window.chrome.webview &&
      typeof window.chrome.webview.postMessage === 'function'
    );

  function post(message) {
    try {
      if (android) window.raygunBridge.postMessage(JSON.stringify(message));
      // An object, not text: the app's own messages are text, and this isn't one of them.
      else if (desktop) window.chrome.webview.postMessage({ raygunBridge: message });
    } catch (error) {
      /* the browser isn't listening */
    }
  }

  var state = Object.freeze({ background: false, call: false, audioActive: false });
  var listeners = [];
  function setState(next) {
    var fresh = Object.freeze({
      background: !!next.background,
      call: !!next.call,
      audioActive: !!next.audioActive,
    });
    var changed =
      fresh.background !== state.background ||
      fresh.call !== state.call ||
      fresh.audioActive !== state.audioActive;
    state = fresh;
    if (!changed) return;
    listeners.slice().forEach(function (listener) {
      try {
        listener(state);
      } catch (error) {
        console.error('[raygun] state listener', error);
      }
    });
  }

  var hub = Object.freeze({
    get state() {
      return state;
    },
    post: post,
    on: function (listener) {
      listeners.push(listener);
      return function () {
        var at = listeners.indexOf(listener);
        if (at >= 0) listeners.splice(at, 1);
      };
    },
  });
  try {
    Object.defineProperty(window, HUB, { value: hub });
  } catch (error) {
    return;
  }

  // ---- What plays ----

  var handlers = {};
  var session = navigator.mediaSession;
  if (session && typeof session.setActionHandler === 'function') {
    var setAction = session.setActionHandler;
    try {
      // The handlers the page sets are kept, so the browser's controls reach them as the
      // system's would.
      session.setActionHandler = function (action, handler) {
        handlers[action] = typeof handler === 'function' ? handler : null;
        schedule();
        return setAction.call(session, action, handler);
      };
    } catch (error) {
      /* left alone */
    }
    try {
      var described = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(session), 'metadata');
      if (described && described.get && described.set) {
        Object.defineProperty(session, 'metadata', {
          configurable: true,
          enumerable: true,
          get: function () {
            return described.get.call(session);
          },
          set: function (value) {
            described.set.call(session, value);
            schedule();
          },
        });
      }
    } catch (error) {
      /* metadata is read when reporting */
    }
  }

  var current = null;
  var last = '';
  var ticking = 0;
  var pending = false;

  function audible(media) {
    return !media.muted && media.volume > 0;
  }

  function artwork(metadata) {
    var list = (metadata && metadata.artwork) || [];
    var best = null;
    var bestSize = -1;
    for (var index = 0; index < list.length; index += 1) {
      var item = list[index] || {};
      var size = parseInt(String(item.sizes || '0').split('x')[0], 10) || 0;
      if (typeof item.src === 'string' && size > bestSize) {
        best = item.src;
        bestSize = size;
      }
    }
    if (!best) return null;
    try {
      var absolute = new URL(best, location.href).href;
      return /^https?:/.test(absolute) ? absolute : null;
    } catch (error) {
      return null;
    }
  }

  function report() {
    pending = false;
    var media = current;
    var playing = !!media && media.isConnected && !media.paused && !media.ended && audible(media);
    var paused =
      !!media &&
      media.isConnected &&
      media.paused &&
      !media.ended &&
      media.currentTime > 0 &&
      audible(media);
    var mode = playing ? 'playing' : paused ? 'paused' : 'none';
    var metadata = null;
    try {
      metadata = session && session.metadata;
    } catch (error) {
      metadata = null;
    }
    var actions = [];
    if (media) actions.push('play', 'pause', 'seekto');
    ['previoustrack', 'nexttrack'].forEach(function (action) {
      if (handlers[action]) actions.push(action);
    });
    var message = {
      type: 'media',
      state: mode,
      title: (metadata && metadata.title) || document.title || '',
      artist: (metadata && metadata.artist) || '',
      album: (metadata && metadata.album) || '',
      artwork: artwork(metadata),
      duration: media && isFinite(media.duration) ? media.duration : null,
      position: media ? media.currentTime : null,
      rate: media ? media.playbackRate : 1,
      actions: actions,
    };
    // Positions move on their own: they're sent with each change and every few seconds.
    var shape = JSON.stringify([
      mode,
      message.title,
      message.artist,
      message.album,
      message.artwork,
      message.duration,
      message.rate,
      actions,
    ]);
    if (shape !== last || mode === 'playing') {
      if (!(mode === 'none' && last === '')) post(message);
      last = mode === 'none' ? '' : shape;
    }
    if (mode === 'playing' && !ticking) {
      ticking = setInterval(report, 5000);
    } else if (mode !== 'playing' && ticking) {
      clearInterval(ticking);
      ticking = 0;
    }
  }

  function schedule() {
    if (pending) return;
    pending = true;
    setTimeout(report, 0);
  }

  [
    'play',
    'playing',
    'pause',
    'ended',
    'emptied',
    'durationchange',
    'ratechange',
    'seeked',
    'volumechange',
    'loadedmetadata',
  ].forEach(function (type) {
    document.addEventListener(
      type,
      function (event) {
        var media = event.target;
        if (!media || (media.tagName !== 'VIDEO' && media.tagName !== 'AUDIO')) return;
        if (type === 'play' || type === 'playing' || !current) current = media;
        if (media === current) schedule();
      },
      true,
    );
  });

  // ---- From the browser ----

  function control(message) {
    var action = message.action;
    var handler = handlers[action === 'stop' && !handlers.stop ? 'pause' : action];
    if (handler) {
      try {
        handler({ action: action, seekTime: message.seekTime });
        return;
      } catch (error) {
        console.error('[raygun] media session handler', error);
      }
    }
    var media = current;
    if (!media) return;
    if (action === 'play') {
      var started = media.play();
      if (started && typeof started.catch === 'function') started.catch(function () {});
    } else if (action === 'pause' || action === 'stop') {
      media.pause();
    } else if (action === 'seekto' && typeof message.seekTime === 'number') {
      media.currentTime = message.seekTime;
    }
  }

  function receive(message) {
    if (!message || typeof message !== 'object') return;
    if (message.type === 'state') setState(message);
    else if (message.type === 'control') control(message);
  }

  if (android) {
    window.raygunBridge.addEventListener('message', function (event) {
      try {
        receive(JSON.parse(event.data));
      } catch (error) {
        /* not one of the browser's */
      }
    });
  } else if (desktop) {
    try {
      Object.defineProperty(window, '__raygunPageReceive', { value: receive });
    } catch (error) {
      /* left out */
    }
  }
  post({ type: 'hello' });
})();
