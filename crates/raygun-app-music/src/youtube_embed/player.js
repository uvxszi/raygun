// Raygun's YouTube player script. An initialization script of Raygun's main window, so it runs at
// document start in every frame there. It does nothing unless it is in YouTube's embedded player,
// inside Raygun's embed host page (`http://localhost:<port>/embed/youtube`) inside Raygun's own
// page. Then it does what the host page's address said, which reaches this frame's address as
// player parameters (`raygun_audio`, `raygun_codec`), and what the host page asks:
//
// - `raygun_codec=h264`: YouTube's player is told this webview plays no VP8, VP9, AV1 or WebM, so
//   it picks H.264. `auto` tells it so only of VP9 and AV1 where the webview has no power-efficient
//   (hardware) decoder for them, as its media capabilities say. Adapted from h264ify
//   (erkserkserks/h264ify, MIT) as NouTube carries it (nonbili/NouTube, AGPL-3.0,
//   `content/h264ify.ts`).
// - `raygun_audio=original`: a video with dubbed or auto-translated tracks plays its original one
//   (NouTube's `playDefaultAudio`; the word "original" in YouTube's languages is NouTube's list in
//   `content/audio.ts`, from Bilalkamal/YouTube-Original-Audio-Track).
// - `segments`: SponsorBlock's parts of the video drawn on YouTube's own seek bar (NouTube's
//   `renderSkipSegments`, `content/sponsorblock.ts`).
//
// Messages: host → here {raygunPlayer: true, type: 'prefs', audio, codec}
//                       {raygunPlayer: true, type: 'segments', videoId,
//                        segments: [{startMs, endMs, category}]}
(function () {
  'use strict';
  if (window.top === window || window.__raygunPlayer) return;
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
  window.__raygunPlayer = true;

  function param(name) {
    var pairs = String(location.search || '')
      .replace(/^\?/, '')
      .split('&');
    for (var index = 0; index < pairs.length; index += 1) {
      var pair = pairs[index].split('=');
      if (decodeURIComponent(pair[0]) === name) return decodeURIComponent(pair[1] || '');
    }
    return null;
  }

  var prefs = {
    audio: param('raygun_audio') || 'default',
    codec: param('raygun_codec') || 'auto',
  };

  // ---- Codecs ----

  /** What every codec but H.264 is called in a type. */
  var NOT_H264 = ['webm', 'vp8', 'vp9', 'vp09', 'av01'];
  /** Codecs a probe found no power-efficient decoder for (`auto`). */
  var slow = [];

  function refused() {
    return prefs.codec === 'h264' ? NOT_H264 : prefs.codec === 'auto' ? slow : [];
  }

  function refuses(type) {
    if (typeof type !== 'string') return false;
    var lower = type.toLowerCase();
    var names = refused();
    for (var index = 0; index < names.length; index += 1) {
      if (lower.indexOf(names[index]) !== -1) return true;
    }
    return false;
  }

  /** `owner[name](type)` answers `refusal` for refused types. */
  function wrap(owner, name, refusal) {
    var original = owner && owner[name];
    if (typeof original !== 'function' || original.__raygun) return null;
    var wrapped = function (type) {
      if (refuses(type)) return typeof refusal === 'function' ? refusal() : refusal;
      return original.apply(this === undefined ? owner : this, arguments);
    };
    wrapped.__raygun = true;
    try {
      owner[name] = wrapped;
    } catch (error) {
      return null;
    }
    return original;
  }

  try {
    wrap(window.HTMLMediaElement && window.HTMLMediaElement.prototype, 'canPlayType', '');
  } catch (error) {
    // Not in this webview.
  }
  wrap(window.MediaSource, 'isTypeSupported', false);
  wrap(window.ManagedMediaSource, 'isTypeSupported', false);

  var capabilities = window.navigator && window.navigator.mediaCapabilities;
  var decodingInfo = null;
  if (capabilities && typeof capabilities.decodingInfo === 'function') {
    var askedInfo = capabilities.decodingInfo;
    decodingInfo = function (config) {
      return askedInfo.call(capabilities, config);
    };
    try {
      capabilities.decodingInfo = function (config) {
        var type = config && config.video && config.video.contentType;
        if (refuses(type)) {
          return Promise.resolve({ supported: false, smooth: false, powerEfficient: false });
        }
        return decodingInfo(config);
      };
    } catch (error) {
      // Read-only here.
    }
  }

  /** `auto`: VP9 and AV1 are refused where only software decodes them. */
  function probe() {
    if (!decodingInfo) return;
    var probes = [
      ['video/webm; codecs="vp09.00.40.08"', ['vp9', 'vp09']],
      ['video/mp4; codecs="av01.0.08M.08"', ['av01']],
    ];
    probes.forEach(function (entry) {
      var asked;
      try {
        asked = decodingInfo({
          type: 'media-source',
          video: {
            contentType: entry[0],
            width: 1920,
            height: 1080,
            bitrate: 4000000,
            framerate: 30,
          },
        });
      } catch (error) {
        return;
      }
      Promise.resolve(asked).then(
        function (info) {
          if (info && info.supported && !info.powerEfficient) slow = slow.concat(entry[1]);
        },
        function () {},
      );
    });
  }
  probe();

  // ---- The original audio track ----

  /** "Original" in YouTube's languages (NouTube's list). */
  var ORIGINAL = [
    'original',
    'originale',
    'оригинал',
    'オリジナル',
    '원본',
    '原始',
    'أصلي',
    'मूल',
    'মূল',
    'asli',
    'orijinal',
    'nguyên bản',
    'ต้นฉบับ',
    'origineel',
    'oryginalny',
    'alkuperäinen',
    'originál',
    'πρωτότυπο',
    'מקורי',
    'eredeti',
    'оригінал',
    'اصلی',
    'asal',
    'orihinal',
    'அசல்',
    'اصل',
    'izvorni',
    'оригинален',
    'izvirnik',
    'originalus',
    'oriģināls',
    'originaal',
    'ორიგინალი',
    'asl',
    'түпнұсқа',
    'բնօրինակ',
    'ដើម',
    'မူရ်း',
    'මුල්',
    'የመጀመሪያው',
    'मौलिक',
    'ຕົ້ນສະບັບ',
    'эх',
    'gwreiddiol',
    'originala',
    'jatorrizko',
    'orixinal',
    'મૂળ',
    'asali',
    'upprunalegt',
    'ಮೂಲ',
    'orîjînal',
    'түп нуска',
    'taketake',
    'യഥാർത്ഥ',
    'मूळ',
    'oriġinali',
    'ମୌଳିକ',
    'ਅਸਲ',
    'اصلي',
    'origjinal',
    'asili',
    'అసలు',
    'asyl',
    'оригиналь',
    'ئەسلى',
    'אריגינעל',
    'atilẹba',
    'okwangempela',
    'oorspronklik',
    'originalni',
    'oarspronklik',
    'bunaidh',
    'tùsail',
    'originalis',
  ];

  /** A track's own details: on it, or on an object it holds (YouTube's names change). */
  function trackInfo(track) {
    if (!track || typeof track !== 'object') return null;
    if (typeof track.name === 'string') return track;
    for (var key in track) {
      var value = track[key];
      if (value && typeof value === 'object' && typeof value.name === 'string') return value;
    }
    return null;
  }

  function isOriginal(info) {
    if (!info) return false;
    if (info.isAutoDubbed === true) return false;
    var name = String(info.name).toLowerCase();
    for (var index = 0; index < ORIGINAL.length; index += 1) {
      if (name.indexOf(ORIGINAL[index]) !== -1) return true;
    }
    return false;
  }

  /** The video whose track was settled, and tries for the one loading. */
  var audioFor = null;
  var audioTries = 0;

  function chooseOriginal(player, videoId) {
    if (!videoId || audioFor === videoId) return;
    var tracks = [];
    try {
      tracks = player.getAvailableAudioTracks ? player.getAvailableAudioTracks() || [] : [];
    } catch (error) {
      tracks = [];
    }
    if (tracks.length < 2) {
      // Not listed yet, or one track only: asked again a few times.
      audioTries += 1;
      if (audioTries > 15) {
        audioFor = videoId;
        audioTries = 0;
      }
      return;
    }
    audioFor = videoId;
    audioTries = 0;
    var original = null;
    for (var index = 0; index < tracks.length && !original; index += 1) {
      if (isOriginal(trackInfo(tracks[index]))) original = tracks[index];
    }
    if (!original) return;
    try {
      var current = trackInfo(player.getAudioTrack ? player.getAudioTrack() : null);
      var wanted = trackInfo(original);
      if (current && wanted && current.id !== undefined && current.id === wanted.id) return;
      player.setAudioTrack(original);
    } catch (error) {
      // The player changed under it; the next video tries again.
    }
  }

  // ---- SponsorBlock's parts on the seek bar ----

  /** SponsorBlock's colours. */
  var COLORS = {
    sponsor: '#00d400',
    selfpromo: '#ffff00',
    interaction: '#cc00ff',
    intro: '#00ffff',
    outro: '#0202ed',
    preview: '#008fd6',
    musicOfftopic: '#ff9900',
    filler: '#7300ff',
  };
  var OVERLAY = '_raygun_sb_segments';
  // The bar spanning the whole video: the embed's, then the newer and older mobile ones.
  var BARS = ['.ytp-progress-list', '.ytChapteredProgressBarHost', '.progress-bar-background'];
  // A chaptered video's bar is in blocks with gaps between them, so a part is drawn piece by
  // piece to keep to the time it covers.
  var CHAPTERS = [
    '.ytp-chapter-hover-container',
    '.ytChapteredProgressBarChapteredPlayerBarChapter',
  ];

  var parts = { videoId: null, segments: [] };

  function bars() {
    var found = [];
    for (var index = 0; index < BARS.length; index += 1) {
      var elements = document.querySelectorAll(BARS[index]);
      for (var at = 0; at < elements.length; at += 1) {
        if (elements[at].offsetWidth > 0) found.push(elements[at]);
      }
    }
    return found;
  }

  function chaptersOf(bar) {
    var elements = [];
    for (var index = 0; index < CHAPTERS.length && !elements.length; index += 1) {
      elements = Array.prototype.slice.call(bar.querySelectorAll(CHAPTERS[index]));
    }
    if (!elements.length) return [{ start: 0, end: 1, left: 0, width: bar.offsetWidth }];
    var total = elements.reduce(function (sum, element) {
      return sum + element.offsetWidth;
    }, 0);
    var chapters = [];
    var start = 0;
    elements.forEach(function (element) {
      var percent = parseFloat(element.style.width);
      var span =
        /%$/.test(element.style.width) && isFinite(percent)
          ? percent / 100
          : element.offsetWidth / (total || 1);
      chapters.push({
        start: start,
        end: start + span,
        left: element.offsetLeft,
        width: element.offsetWidth,
      });
      start += span;
    });
    chapters[chapters.length - 1].end = 1;
    return chapters;
  }

  function overlay(segments, durationMs, chapters) {
    var box = document.createElement('div');
    box.className = OVERLAY;
    box.style.cssText =
      'position:absolute;top:0;left:0;width:100%;height:100%;pointer-events:none;z-index:1';
    segments.forEach(function (segment) {
      var start = Math.max(0, Math.min(1, segment.startMs / durationMs));
      var end = Math.max(start, Math.min(1, segment.endMs / durationMs));
      chapters.forEach(function (chapter) {
        var from = Math.max(start, chapter.start);
        var to = Math.min(end, chapter.end);
        var span = chapter.end - chapter.start;
        if (to <= from || span <= 0) return;
        var width = ((to - from) / span) * chapter.width;
        if (width < 0.5) return;
        var piece = document.createElement('div');
        piece.style.cssText =
          'position:absolute;top:0;height:100%;opacity:0.85;left:' +
          (chapter.left + ((from - chapter.start) / span) * chapter.width) +
          'px;width:' +
          Math.max(1, width) +
          'px;background:' +
          (COLORS[segment.category] || COLORS.sponsor);
        box.appendChild(piece);
      });
    });
    return box;
  }

  function clearParts() {
    var drawn = document.querySelectorAll('.' + OVERLAY);
    for (var index = 0; index < drawn.length; index += 1) {
      var bar = drawn[index].parentElement;
      if (!bar) continue;
      bar.removeAttribute('data-raygun-sb');
      bar.removeChild(drawn[index]);
    }
  }

  function draw(player, videoId) {
    if (!parts.segments.length || parts.videoId !== videoId) {
      if (document.querySelector('.' + OVERLAY)) clearParts();
      return;
    }
    var seconds = 0;
    try {
      seconds = player.getDuration ? player.getDuration() : 0;
    } catch (error) {
      seconds = 0;
    }
    if (!(seconds > 0)) return;
    var durationMs = seconds * 1000;
    var shape = parts.segments
      .map(function (segment) {
        return segment.category + ':' + segment.startMs + ':' + segment.endMs;
      })
      .join(',');
    bars().forEach(function (bar) {
      var chapters = chaptersOf(bar);
      var key =
        videoId +
        ':' +
        durationMs +
        ':' +
        shape +
        ':' +
        bar.offsetWidth +
        ':' +
        chapters
          .map(function (chapter) {
            return chapter.left + '/' + chapter.width;
          })
          .join(',');
      var drawn = bar.querySelector('.' + OVERLAY);
      // YouTube draws its controls again and resizes them: redrawn when the shape changed.
      if (drawn && bar.getAttribute('data-raygun-sb') === key) return;
      if (drawn) bar.removeChild(drawn);
      if (window.getComputedStyle && window.getComputedStyle(bar).position === 'static') {
        bar.style.position = 'relative';
      }
      bar.appendChild(overlay(parts.segments, durationMs, chapters));
      bar.setAttribute('data-raygun-sb', key);
    });
  }

  // ---- Following the player ----

  function playerNow() {
    var player = document.getElementById('movie_player');
    if (!player || typeof player.getVideoData !== 'function') return null;
    var data = null;
    try {
      data = player.getVideoData();
    } catch (error) {
      data = null;
    }
    return { player: player, videoId: (data && data.video_id) || null };
  }

  function tick() {
    var now = playerNow();
    if (!now) return;
    if (prefs.audio === 'original') chooseOriginal(now.player, now.videoId);
    draw(now.player, now.videoId);
  }

  setInterval(tick, 1000);
  window.addEventListener('resize', function () {
    setTimeout(tick, 200);
  });

  window.addEventListener('message', function (event) {
    if (event.source !== window.parent || event.origin !== hostOrigin) return;
    var data = event.data;
    if (!data || data.raygunPlayer !== true) return;
    if (data.type === 'prefs') {
      if (data.audio === 'original' || data.audio === 'default') {
        if (data.audio !== prefs.audio) audioFor = null;
        prefs.audio = data.audio;
      }
      if (data.codec === 'h264' || data.codec === 'auto') prefs.codec = data.codec;
    } else if (data.type === 'segments') {
      parts = {
        videoId: typeof data.videoId === 'string' ? data.videoId : null,
        segments: Array.isArray(data.segments)
          ? data.segments.filter(function (segment) {
              return (
                segment &&
                isFinite(segment.startMs) &&
                isFinite(segment.endMs) &&
                segment.endMs > segment.startMs
              );
            })
          : [],
      };
      tick();
    }
  });
})();
