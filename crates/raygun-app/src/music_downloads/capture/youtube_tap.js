// Raygun's YouTube tap (M46.52). An initialization script of Raygun's main window, so it runs at
// document start in every frame there. It does nothing unless it is in YouTube's embedded player,
// inside Raygun's embed host page (`http://localhost:<port>/embed/youtube`) inside Raygun's own
// page, and that host asks it to tap. Then it taps the player's <video>: its sound goes to a
// script processor (never to the speakers), and pieces of 16-bit PCM go to Raygun's page, each
// saying where in the video it starts.
//
// Two ways, as Raygun asks:
// - `graph` (desktop): the element's sound through createMediaElementSource into a context at
//   48 kHz × the speed, its pitch left free, so a 2× capture keeps all of the sound.
// - `muted` (Android): the element is muted, so it doesn't take Android's audio focus from
//   Raygun's own player, and its sound is read from captureStream(), which muting doesn't
//   silence. A webview that gives a muted element's capture only zeros says `silent`.
//
// Messages: host → here {raygunTap: true, type: 'start' | 'stop', session, speed, mode}
//           here → top  {raygunTap: true, session, type: 'armed' | 'attached' | 'sound' |
//                        'stalled' | 'ended' | 'failed', startMs, rate, channels, speed, pcm,
//                        durationMs, error}
(function () {
  'use strict';
  if (window.top === window || window.__raygunTap) return;
  var origin = location.origin;
  if (origin !== 'https://www.youtube.com' && origin !== 'https://www.youtube-nocookie.com') return;
  var ancestors = location.ancestorOrigins;
  if (!ancestors || ancestors.length < 2) return;
  var hostOrigin = ancestors[0];
  var topOrigin = ancestors[ancestors.length - 1];
  if (!/^http:\/\/localhost:\d+$/.test(hostOrigin)) return;
  if (
    !/^(https?:\/\/tauri\.localhost|tauri:\/\/localhost|http:\/\/localhost:\d+)$/.test(topOrigin)
  ) {
    return;
  }
  window.__raygunTap = true;

  /** The sound's own rate; a graph context runs at this times the speed. */
  var SOUND_RATE = 48000;
  /** Frames per processor callback. */
  var BLOCK = 8192;
  /** A muted tap that heard only zeros for this long of playing hears nothing, ms. */
  var SILENT_MS = 12000;
  /** After `ended`, the sound still on its way is waited for this long at most, ms. */
  var FLUSH_MS = 1500;

  var current = null;

  function post(message, transfer) {
    message.raygunTap = true;
    try {
      window.top.postMessage(message, topOrigin, transfer || []);
    } catch (error) {
      // Raygun's page went away.
    }
  }

  function findVideo() {
    return document.querySelector('video.html5-main-video') || document.querySelector('video');
  }

  /** YouTube shows an ad: its sound isn't the video's. */
  function adShowing() {
    return !!document.querySelector('.ad-showing, .ad-interrupting');
  }

  function durationOf(tap) {
    var seconds = tap.video ? tap.video.duration : NaN;
    return isFinite(seconds) && seconds > 0 ? seconds * 1000 : null;
  }

  function report(tap, type, extra) {
    var message = {
      type: type,
      session: tap.id,
      startMs: tap.video ? tap.video.currentTime * 1000 : 0,
      rate: tap.context ? tap.context.sampleRate : SOUND_RATE,
      channels: 2,
      speed: tap.speed,
      durationMs: durationOf(tap),
      error: null,
    };
    for (var key in extra) message[key] = extra[key];
    post(message, extra && extra.pcm ? [extra.pcm] : []);
  }

  function fail(tap, error) {
    if (tap.stopped) return;
    report(tap, 'failed', { error: error });
    stop(tap.id);
  }

  /** The run being heard ends at the context's time now. */
  function endRun(tap) {
    var run = tap.runs[tap.runs.length - 1];
    if (run && run.until === null) run.until = tap.context.currentTime;
  }

  /** Playing at the asked speed, the video's own sound: a run is open. */
  function heardRight(tap) {
    var video = tap.video;
    if (video.paused || video.seeking || video.ended || video.readyState < 3) return false;
    if (adShowing()) return false;
    if (Math.abs(video.playbackRate - tap.speed) > 0.01) {
      try {
        video.playbackRate = tap.speed;
      } catch (error) {
        // Not now.
      }
      return false;
    }
    return true;
  }

  function openRun(tap) {
    var run = tap.runs[tap.runs.length - 1];
    if (run && run.until === null) return;
    tap.runs.push({
      streamMs: tap.video.currentTime * 1000,
      contextTime: tap.context.currentTime,
      until: null,
    });
  }

  /** Interleaved 16-bit samples of frames `from..to` of a block. */
  function samplesOf(input, from, to) {
    var channels = Math.max(1, Math.min(2, input.numberOfChannels));
    var out = new Int16Array((to - from) * 2);
    var left = input.getChannelData(0);
    var right = channels > 1 ? input.getChannelData(1) : left;
    for (var frame = from, at = 0; frame < to; frame += 1, at += 2) {
      var l = left[frame] < -1 ? -1 : left[frame] > 1 ? 1 : left[frame];
      var r = right[frame] < -1 ? -1 : right[frame] > 1 ? 1 : right[frame];
      out[at] = Math.round(l * 32767);
      out[at + 1] = Math.round(r * 32767);
    }
    return out;
  }

  function allZero(samples) {
    for (var index = 0; index < samples.length; index += 1) if (samples[index] !== 0) return false;
    return true;
  }

  /** Sends a piece of sound, held back while a muted tap has heard only zeros. */
  function send(tap, startMs, samples) {
    if (tap.mode === 'muted' && !tap.heard) {
      if (allZero(samples)) {
        tap.held.push([startMs, samples]);
        tap.zeroMs += (samples.length / 2 / tap.context.sampleRate) * 1000 * tap.speed;
        if (tap.zeroMs >= SILENT_MS) fail(tap, 'silent');
        return;
      }
      tap.heard = true;
      var held = tap.held;
      tap.held = [];
      for (var index = 0; index < held.length; index += 1)
        send(tap, held[index][0], held[index][1]);
    }
    report(tap, 'sound', { startMs: startMs, pcm: samples.buffer });
  }

  function onBlock(tap, event) {
    var output = event.outputBuffer;
    for (var channel = 0; channel < output.numberOfChannels; channel += 1) {
      output.getChannelData(channel).fill(0);
    }
    if (tap.stopped) return;
    var input = event.inputBuffer;
    var rate = input.sampleRate;
    var frames = input.length;
    // The block's frames came in the two blocks' time before the one it plays at; consecutive
    // blocks follow on from each other unless the page fell behind.
    var start = event.playbackTime - (2 * frames - 128) / rate;
    if (tap.blockEnd !== null && Math.abs(start - tap.blockEnd) < frames / 2 / rate) {
      start = tap.blockEnd;
    }
    var end = start + frames / rate;
    tap.blockEnd = end;
    if (tap.mode === 'muted' && !tap.video.muted) tap.video.muted = true;
    if (tap.ending === null) {
      if (heardRight(tap)) openRun(tap);
      else endRun(tap);
    }
    for (var index = 0; index < tap.runs.length && !tap.stopped; index += 1) {
      var run = tap.runs[index];
      var from = Math.max(start, run.contextTime);
      var to = Math.min(end, run.until === null ? Infinity : run.until);
      if (to <= from) continue;
      var first = Math.round((from - start) * rate);
      var last = Math.min(frames, Math.round((to - start) * rate));
      if (last <= first) continue;
      var startMs = run.streamMs + (from - run.contextTime) * 1000 * tap.speed;
      send(tap, startMs, samplesOf(input, first, last));
    }
    if (tap.stopped) return;
    // Runs that ended before this block are done with.
    tap.runs = tap.runs.filter(function (run) {
      return run.until === null || run.until > end;
    });
    if (tap.ending !== null && (end >= tap.ending || Date.now() - tap.endedAt > FLUSH_MS)) {
      tap.ending = null;
      report(tap, 'ended', {});
      stop(tap.id);
    }
  }

  /** The muted way: each audio track the element's capture gets feeds the processor. */
  function listenToCapture(tap, processor) {
    var capture = tap.video.captureStream ? tap.video.captureStream() : null;
    if (!capture) {
      fail(tap, 'This webview can’t capture YouTube’s player');
      return;
    }
    var sources = [];
    var connect = function () {
      for (var index = 0; index < sources.length; index += 1) sources[index].disconnect();
      sources = [];
      var tracks = capture.getAudioTracks();
      for (var at = 0; at < tracks.length; at += 1) {
        var source = tap.context.createMediaStreamSource(new MediaStream([tracks[at]]));
        source.connect(processor);
        sources.push(source);
      }
    };
    capture.addEventListener('addtrack', connect);
    capture.addEventListener('removetrack', connect);
    tap.nodes.push({
      disconnect: function () {
        capture.removeEventListener('addtrack', connect);
        capture.removeEventListener('removetrack', connect);
        for (var index = 0; index < sources.length; index += 1) sources[index].disconnect();
      },
    });
    connect();
  }

  function attach(tap, video) {
    if (tap.stopped) return;
    tap.video = video;
    // Faster only with its pitch left free: the samples are read back at its own pace.
    if ('preservesPitch' in video) video.preservesPitch = false;
    else if ('webkitPreservesPitch' in video) video.webkitPreservesPitch = false;
    else tap.speed = 1;
    var context = null;
    try {
      context = new AudioContext({
        sampleRate: tap.mode === 'graph' ? SOUND_RATE * tap.speed : SOUND_RATE,
      });
    } catch (error) {
      try {
        context = new AudioContext();
      } catch (failure) {
        fail(tap, 'This webview has no audio context for YouTube’s player');
        return;
      }
      if (tap.mode === 'graph') tap.speed = 1;
    }
    tap.context = context;
    var processor = context.createScriptProcessor(BLOCK, 2, 2);
    processor.onaudioprocess = function (event) {
      onBlock(tap, event);
    };
    processor.connect(context.destination);
    tap.nodes.push(processor);
    try {
      if (tap.mode === 'muted') {
        video.muted = true;
        listenToCapture(tap, processor);
      } else {
        var source = context.createMediaElementSource(video);
        source.connect(processor);
        tap.nodes.push(source);
      }
    } catch (error) {
      fail(tap, error && error.message ? error.message : String(error));
      return;
    }
    var listen = function (type, handler) {
      video.addEventListener(type, handler);
      tap.listeners.push([type, handler]);
    };
    listen('playing', function () {
      if (tap.ending === null && heardRight(tap)) openRun(tap);
    });
    listen('waiting', function () {
      endRun(tap);
      report(tap, 'stalled', {});
    });
    listen('pause', function () {
      endRun(tap);
    });
    listen('seeking', function () {
      endRun(tap);
    });
    listen('ratechange', function () {
      if (Math.abs(video.playbackRate - tap.speed) > 0.01) endRun(tap);
    });
    listen('ended', function () {
      endRun(tap);
      tap.ending = tap.context.currentTime;
      tap.endedAt = Date.now();
    });
    context.resume().catch(function () {
      // Resumed when the player plays.
    });
    if (!video.paused && heardRight(tap)) openRun(tap);
    report(tap, 'attached', {});
  }

  function start(request) {
    if (current) stop(current.id);
    var tap = {
      id: String(request.session),
      speed: request.speed > 0 ? request.speed : 1,
      mode: request.mode === 'muted' ? 'muted' : 'graph',
      video: null,
      context: null,
      nodes: [],
      listeners: [],
      runs: [],
      observer: null,
      stopped: false,
      blockEnd: null,
      ending: null,
      endedAt: 0,
      heard: false,
      held: [],
      zeroMs: 0,
    };
    current = tap;
    report(tap, 'armed', {});
    var video = findVideo();
    if (video) {
      attach(tap, video);
      return;
    }
    tap.observer = new MutationObserver(function () {
      var found = findVideo();
      if (!found || tap.stopped) return;
      tap.observer.disconnect();
      tap.observer = null;
      attach(tap, found);
    });
    tap.observer.observe(document.documentElement || document, { childList: true, subtree: true });
  }

  function stop(id) {
    var tap = current;
    if (!tap || tap.id !== String(id) || tap.stopped) return;
    tap.stopped = true;
    current = null;
    if (tap.observer) tap.observer.disconnect();
    for (var index = 0; index < tap.listeners.length; index += 1) {
      tap.video.removeEventListener(tap.listeners[index][0], tap.listeners[index][1]);
    }
    for (var at = 0; at < tap.nodes.length; at += 1) {
      try {
        tap.nodes[at].disconnect();
      } catch (error) {
        // Gone already.
      }
    }
    if (tap.video) {
      try {
        tap.video.pause();
      } catch (error) {
        // Gone already.
      }
    }
    if (tap.context) tap.context.close().catch(function () {});
  }

  window.addEventListener('message', function (event) {
    if (event.source !== window.parent || event.origin !== hostOrigin) return;
    var data = event.data;
    if (!data || data.raygunTap !== true || data.session == null) return;
    if (data.type === 'start') start(data);
    else if (data.type === 'stop') stop(data.session);
  });
})();
