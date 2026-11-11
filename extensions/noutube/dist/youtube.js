// dev.raygun.noutube 1.0.0. Built by raygun-ext from src/; do not edit.
"use strict";
(() => {
  // src/lib/json.ts
  var isObject = (value) => typeof value === "object" && value !== null;
  var pick = (value, ...path) => {
    let current = value;
    for (const key of path) {
      if (!isObject(current)) return void 0;
      current = current[key];
    }
    return current;
  };
  var pickString = (value, ...path) => {
    const found = pick(value, ...path);
    return typeof found === "string" ? found : void 0;
  };
  var pickArray = (value, ...path) => {
    const found = pick(value, ...path);
    return Array.isArray(found) ? found : [];
  };
  var textOf = (node) => {
    if (typeof node === "string") return node;
    if (!isObject(node)) return "";
    if (typeof node.simpleText === "string") return node.simpleText;
    if (typeof node.content === "string") return node.content;
    if (Array.isArray(node.runs)) {
      return node.runs.map((run) => pickString(run, "text") ?? "").join("");
    }
    return "";
  };

  // src/page.ts
  var flag = (key, fallback) => {
    const value = raygunPage.settings[key];
    return typeof value === "boolean" ? value : fallback;
  };
  var textSetting = (key, fallback = "") => {
    const value = raygunPage.settings[key];
    return typeof value === "string" ? value : fallback;
  };
  var isMusic = location.hostname === "music.youtube.com";
  var isMobileSite = location.hostname === "m.youtube.com";
  var isAndroid = raygunPage.platform === "android";
  var isMobileAgent = /Android|Mobile/i.test(navigator.userAgent);
  var pageFetch = window.fetch.bind(window);
  var latestState = raygunPage.state ?? {
    background: false,
    call: false,
    audioActive: false
  };
  raygunPage.on("state", (state) => {
    latestState = state;
  });
  var pageState = () => latestState;
  function whenRoot(run) {
    const root = document.documentElement;
    if (root) {
      run(root);
      return;
    }
    const observer = new MutationObserver(() => {
      const element = document.documentElement;
      if (!element) return;
      observer.disconnect();
      run(element);
    });
    observer.observe(document, { childList: true });
  }
  function whenBody(run) {
    if (document.body) {
      run(document.body);
      return;
    }
    document.addEventListener("DOMContentLoaded", () => run(document.body), { once: true });
  }
  function addStyle(id, css) {
    whenRoot((root) => {
      let style = document.getElementById(id);
      if (!style) {
        style = document.createElement("style");
        style.id = id;
        (document.head || root).appendChild(style);
      }
      style.textContent = css;
    });
  }
  var NAVIGATION_EVENTS = [
    "yt-navigate-finish",
    "yt-page-data-updated",
    "state-navigateend",
    "popstate"
  ];
  function onNavigate(listener) {
    let scheduled = false;
    const schedule = () => {
      if (scheduled) return;
      scheduled = true;
      setTimeout(() => {
        scheduled = false;
        listener();
      }, 0);
    };
    for (const name of NAVIGATION_EVENTS) {
      window.addEventListener(name, schedule);
      document.addEventListener(name, schedule);
    }
  }
  function coalesce(run, wait) {
    let timer;
    return () => {
      if (timer !== void 0) return;
      timer = setTimeout(() => {
        timer = void 0;
        run();
      }, wait);
    };
  }
  var ytcfgValue = (key) => {
    const ytcfg = window.ytcfg;
    try {
      return ytcfg?.get?.(key) ?? ytcfg?.data_?.[key];
    } catch {
      return ytcfg?.data_?.[key];
    }
  };
  var VIDEO_ID = /^[A-Za-z0-9_-]{11}$/;
  var watchVideoId = (address = location.href) => {
    try {
      const url = new URL(address, location.href);
      if (url.pathname !== "/watch") return "";
      const id = url.searchParams.get("v") ?? "";
      return VIDEO_ID.test(id) ? id : "";
    } catch {
      return "";
    }
  };
  var memoryStorage = /* @__PURE__ */ new Map();
  var nouStorage = {
    getItem(key) {
      try {
        return localStorage.getItem(key);
      } catch {
        return memoryStorage.get(key) ?? null;
      }
    },
    setItem(key, value) {
      try {
        localStorage.setItem(key, value);
      } catch {
        memoryStorage.set(key, value);
      }
    },
    removeItem(key) {
      try {
        localStorage.removeItem(key);
      } catch {
        memoryStorage.delete(key);
      }
    }
  };

  // src/player.ts
  var PlayerState = { ended: 0, playing: 1, paused: 2, buffering: 3 };
  var videoListeners = [];
  var timeListeners = [];
  var stateListeners = [];
  var onVideoChange = (listener) => void videoListeners.push(listener);
  var onTimeUpdate = (listener) => void timeListeners.push(listener);
  var onPlayerState = (listener) => void stateListeners.push(listener);
  var report = (where, error) => console.error(`[YouTube enhancements] ${where}:`, error);
  function getPlayer() {
    const element = document.getElementById("movie_player");
    return element ? element : void 0;
  }
  function getVideoElement(player2) {
    const video = player2?.querySelector("video");
    return video instanceof HTMLVideoElement ? video : null;
  }
  function readVideo(player2) {
    let response;
    try {
      response = player2?.getPlayerResponse?.();
    } catch {
      return void 0;
    }
    const details = pick(response, "videoDetails");
    const id = pickString(details, "videoId");
    if (!id) return void 0;
    return {
      id,
      title: pickString(details, "title") ?? "",
      author: pickString(details, "author") ?? "",
      lengthSeconds: Number(pick(details, "lengthSeconds")) || 0,
      live: Boolean(pick(response, "playabilityStatus", "liveStreamability")),
      liveContent: pick(details, "isLiveContent") === true
    };
  }
  function playerVideoId(player2) {
    try {
      const data = player2?.getVideoData?.();
      return isObject(data) && typeof data.video_id === "string" && data.video_id || "";
    } catch {
      return "";
    }
  }
  function playsMainVideo(player2) {
    if (!player2?.isConnected) return false;
    if (isMusic || location.pathname === "/watch") return true;
    return !isMobileSite && !player2.closest("ytd-video-preview, #inline-preview-player");
  }
  var player;
  var media = null;
  var unbindMedia = () => {
  };
  var unbindPlayer = () => {
  };
  var videoId = "";
  function emit(listeners, ...args) {
    for (const listener of listeners) {
      try {
        listener(...args);
      } catch (error) {
        report("player", error);
      }
    }
  }
  function checkVideo() {
    const video = player ? readVideo(player) : void 0;
    const id = video?.id ?? "";
    if (id === videoId) return;
    videoId = id;
    emit(videoListeners, video, player);
  }
  function syncMedia() {
    const next = getVideoElement(player);
    if (next === media) return;
    unbindMedia();
    media = next;
    if (!next || !player) return;
    const owner = player;
    const onTime = () => {
      if (player === owner && media === next) emit(timeListeners, owner, next);
    };
    next.addEventListener("timeupdate", onTime);
    next.addEventListener("play", onTime);
    next.addEventListener("seeked", onTime);
    unbindMedia = () => {
      next.removeEventListener("timeupdate", onTime);
      next.removeEventListener("play", onTime);
      next.removeEventListener("seeked", onTime);
    };
  }
  function bind(element) {
    if (element === player) {
      syncMedia();
      return;
    }
    unbindPlayer();
    player = element;
    const onStateChange = (value) => {
      if (player !== element) return;
      syncMedia();
      if (typeof value === "number") emit(stateListeners, element, value);
      checkVideo();
    };
    const api = element;
    api.addEventListener("onStateChange", onStateChange);
    unbindPlayer = () => {
      unbindMedia();
      unbindMedia = () => {
      };
      media = null;
      try {
        api.removeEventListener?.("onStateChange", onStateChange);
      } catch {
      }
    };
    syncMedia();
    checkVideo();
  }
  function check() {
    const found = getPlayer();
    if (found) {
      bind(found);
    } else if (player && !player.isConnected) {
      unbindPlayer();
      unbindPlayer = () => {
      };
      player = void 0;
      checkVideo();
    }
  }
  var watching = false;
  function watchPlayer() {
    if (watching) return;
    watching = true;
    whenRoot((root) => {
      check();
      new MutationObserver(check).observe(root, { childList: true, subtree: true });
    });
  }

  // src/audio.ts
  var l10nOriginal = {
    en: "original",
    es: "original",
    fr: "originale",
    de: "original",
    it: "originale",
    pt: "original",
    ru: "оригинал",
    ja: "オリジナル",
    ko: "원본",
    zh: "原始",
    ar: "أصلي",
    hi: "मूल",
    bn: "মূল",
    id: "asli",
    tr: "orijinal",
    vi: "nguyên bản",
    th: "ต้นฉบับ",
    nl: "origineel",
    pl: "oryginalny",
    sv: "original",
    fi: "alkuperäinen",
    no: "original",
    da: "original",
    cs: "originál",
    el: "πρωτότυπο",
    he: "מקורי",
    ro: "original",
    hu: "eredeti",
    uk: "оригінал",
    fa: "اصلی",
    ms: "asal",
    tl: "orihinal",
    ta: "அசல்",
    ur: "اصل",
    hr: "izvorni",
    bg: "оригинален",
    sk: "originál",
    sl: "izvirnik",
    sr: "оригинал",
    lt: "originalus",
    lv: "oriģināls",
    et: "originaal",
    ka: "ორიგინალი",
    az: "orijinal",
    uz: "asl",
    kk: "түпнұсқа",
    hy: "բնօրինակ",
    km: "ដើម",
    my: "မူရ်း",
    si: "මුල්",
    am: "የመጀመሪያው",
    ne: "मौलिक",
    lo: "ຕົ້ນສະບັບ",
    mn: "эх",
    bo: "དངོས་གཞི།",
    cy: "gwreiddiol",
    eo: "originala",
    eu: "jatorrizko",
    gl: "orixinal",
    gu: "મૂળ",
    ha: "asali",
    ig: "izugbe",
    is: "upprunalegt",
    jv: "asli",
    kn: "ಮೂಲ",
    ku: "orîjînal",
    ky: "түп нуска",
    lb: "original",
    mi: "taketake",
    ml: "യഥാർത്ഥ",
    mr: "मूळ",
    mt: "oriġinali",
    or: "ମୌଳିକ",
    pa: "ਅਸਲ",
    ps: "اصلي",
    sd: "اصل",
    sm: "muamua",
    sn: "chaicho",
    so: "asalka ah",
    sq: "origjinal",
    su: "asli",
    sw: "asili",
    te: "అసలు",
    tk: "asyl",
    tt: "оригиналь",
    ug: "ئەسلى",
    yi: "אריגינעל",
    yo: "atilẹba",
    zu: "okwangempela",
    af: "oorspronklik",
    bs: "originalni",
    ca: "original",
    ceb: "orihinal",
    co: "uriginale",
    fy: "oarspronklik",
    ga: "bunaidh",
    gd: "tùsail",
    haw: "kumu",
    hmn: "thawj",
    ht: "orijinal",
    ia: "original",
    ie: "original",
    io: "originala",
    kw: "gwredhek",
    la: "originalis",
    ln: "ya liboso",
    mg: "tany am-boalohany",
    mk: "оригинален",
    ny: "yoyamba",
    oc: "original",
    qu: "qallariy",
    rm: "original",
    rw: "cy'umwimerere",
    sa: "मूल",
    sc: "originale",
    st: "ea pele",
    tg: "аслӣ",
    to: "ʻuluaki",
    wo: "njëkk",
    xh: "yokuqala",
    "zh-tw": "原始"
  };
  var originalLabels = [
    ...new Set(Object.values(l10nOriginal).map((label) => label.toLocaleLowerCase()))
  ];
  var isOriginal = (name) => {
    const lower = name.toLocaleLowerCase();
    return originalLabels.some((label) => lower.includes(label));
  };
  function trackName(track) {
    if (!isObject(track)) return void 0;
    const infos = Object.values(track).filter(
      (value) => isObject(value) && "isDefault" in value && typeof value.name === "string"
    );
    const info = infos.find((value) => isObject(value) && "isAutoDubbed" in value) ?? infos[0];
    return isObject(info) && typeof info.name === "string" && info.name ? info.name : void 0;
  }
  function audioTracks(player2) {
    try {
      const tracks = player2.getAvailableAudioTracks?.();
      return Array.isArray(tracks) ? tracks : [];
    } catch {
      return [];
    }
  }
  var handled = "";
  var attemptsFor = "";
  var attempts = 0;
  function pickOriginal(player2) {
    const videoId2 = playerVideoId(player2);
    if (!videoId2 || videoId2 === handled || !playsMainVideo(player2)) return;
    if (attemptsFor !== videoId2) {
      attemptsFor = videoId2;
      attempts = 0;
    }
    const tracks = audioTracks(player2);
    if (tracks.length < 2) {
      if (++attempts >= 40) handled = videoId2;
      return;
    }
    handled = videoId2;
    const current = trackName(player2.getAudioTrack?.());
    if (current && isOriginal(current)) return;
    const original = tracks.find((track) => {
      const name = trackName(track);
      return name !== void 0 && isOriginal(name);
    });
    if (original) player2.setAudioTrack?.(original);
  }
  function installOriginalAudio() {
    onVideoChange((_video, player2) => {
      if (player2) pickOriginal(player2);
    });
    onTimeUpdate((player2) => pickOriginal(player2));
  }

  // src/author-names.ts
  var fixEncodedAuthorNames = () => {
    if (location.pathname !== "/watch") return;
    let author;
    try {
      author = pickString(getPlayer()?.getPlayerResponse?.(), "videoDetails", "author");
    } catch {
      return;
    }
    if (!author) return;
    document.querySelectorAll(
      "ytm-slim-video-information-renderer .slim-video-information-channel-name .ytAttributedStringHost, ytm-slim-owner-renderer .ytAttributedStringHost"
    ).forEach((label) => {
      if (!/%[0-9a-f]{2}/i.test(label.textContent || "")) return;
      const parts = label.querySelectorAll(":scope > span");
      const last = parts[parts.length - 1];
      const name = parts.length > 1 && parts[0]?.textContent === "@" && last ? last : label;
      const text = name === label ? "@" + author : author;
      if (name.textContent !== text) name.textContent = text;
    });
  };
  function installEncodedAuthorNameFix() {
    const fix = coalesce(fixEncodedAuthorNames, 250);
    whenRoot((root) => {
      new MutationObserver(fix).observe(root, { childList: true, subtree: true });
      fixEncodedAuthorNames();
    });
  }

  // src/lib/autoplay.ts
  var restoreKey = "nou:autoplay-restore";
  var toggleSelector = [
    ".ytwAutonavToggleButtonHost",
    ".ytp-autonav-toggle-button",
    "ytm-autonav-toggle-button-renderer button",
    'ytm-autonav-toggle-button-renderer [role="switch"]'
  ].join(", ");
  var clicking = false;
  var lastClicks = /* @__PURE__ */ new WeakMap();
  function applyAutoplayPolicy(root, storage, disabled) {
    if (clicking) return;
    if (!disabled && storage.getItem(restoreKey) === null) return;
    for (const toggle of root.querySelectorAll(toggleSelector)) {
      const value = toggle.getAttribute("aria-checked") ?? toggle.getAttribute("aria-pressed");
      if (value !== "true" && value !== "false") continue;
      const enabled = value === "true";
      const original = storage.getItem(restoreKey);
      if (disabled && original === null) storage.setItem(restoreKey, value);
      const desired = disabled ? false : original === "true" ? true : original === "false" ? false : void 0;
      if (desired === void 0) return;
      if (enabled !== desired) {
        const now = Date.now();
        const last = lastClicks.get(toggle);
        if (last !== void 0 && now - last < 1e3) return;
        lastClicks.set(toggle, now);
        clicking = true;
        try {
          toggle.click();
        } finally {
          clicking = false;
        }
        return;
      }
      if (!disabled) storage.removeItem(restoreKey);
      return;
    }
  }

  // src/autoplay.ts
  function installAutoplayPolicy(disabled) {
    if (!disabled && nouStorage.getItem(restoreKey) === null) return;
    const apply = () => applyAutoplayPolicy(document, nouStorage, disabled);
    const later = coalesce(apply, 1e3);
    whenRoot((root) => {
      apply();
      new MutationObserver(later).observe(root, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ["aria-checked", "aria-pressed"]
      });
    });
    onPlayerState(() => apply());
  }

  // src/media-session.ts
  var hooks = /* @__PURE__ */ new Map();
  var wrapped;
  function hookMediaAction(action, hook) {
    hooks.set(action, [...hooks.get(action) ?? [], hook]);
  }
  function wrapMediaSession() {
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
            console.error("[YouTube enhancements] media controls:", error);
          }
        }
        handler(details);
      });
    };
    session.__nouWrapped = true;
    wrapped = session;
  }
  var mediaSessionWrapped = () => wrapped !== void 0;

  // src/background-guard.ts
  var userPaused = false;
  var wasPlaying = false;
  var pausedByInterruption = false;
  var pausedTicks = 0;
  var resumeAttempts = 0;
  var isBackground = () => pageState().background;
  var canAutoResume = () => {
    const state = pageState();
    return !state.call && !state.audioActive;
  };
  function confirmYouThereDialogs() {
    for (const selector of ["ytmusic-you-there-renderer", "ytm-confirm-dialog-renderer"]) {
      const dialog = document.querySelector(selector);
      if (!dialog) continue;
      const buttons = dialog.querySelectorAll("button");
      const [only] = buttons;
      if (buttons.length === 1 && only) {
        only.click();
        return true;
      }
    }
    return false;
  }
  function guardMediaSessionHandlers() {
    hookMediaAction("pause", () => {
      userPaused = true;
      return false;
    });
    hookMediaAction("play", () => {
      userPaused = false;
      return false;
    });
    hookMediaAction("stop", () => {
      userPaused = true;
      return false;
    });
  }
  var inPlayer = (event) => {
    const player2 = getPlayer();
    return event.target instanceof Node && Boolean(player2?.contains(event.target));
  };
  function installBackgroundGuard() {
    if (!mediaSessionWrapped()) return;
    document.addEventListener(
      "pause",
      (event) => {
        if (!isBackground() && inPlayer(event)) userPaused = true;
      },
      true
    );
    for (const type of ["play", "playing"]) {
      document.addEventListener(
        type,
        (event) => {
          if (!inPlayer(event)) return;
          userPaused = false;
          wasPlaying = true;
        },
        true
      );
    }
    setInterval(() => {
      const player2 = getPlayer();
      if (!player2?.getPlayerState) return;
      const state = player2.getPlayerState();
      if (state === PlayerState.playing) {
        userPaused = false;
        wasPlaying = true;
        pausedByInterruption = false;
        pausedTicks = 0;
        resumeAttempts = 0;
        return;
      }
      if (!isBackground()) {
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
        if (pausedTicks >= 6) pausedByInterruption = true;
        return;
      }
      resumeAttempts++;
      confirmYouThereDialogs();
      player2.playVideo?.();
    }, 5e3);
  }
  function keepSessionActive() {
    setInterval(() => {
      window._lact = Date.now();
    }, 60 * 1e3);
  }

  // src/lib/blocklist.ts
  var isBlocklistEmpty = (blocklist) => !blocklist || !blocklist.channels.length && !blocklist.keywords.length;
  var normalizeBlocklistValue = (value) => value.trim().replace(/^https?:\/\/(www\.|m\.)?youtube\.com\//i, "").replace(/^\/+/, "").replace(/^(c|channel|user)\//i, "").replace(/^\/+/, "").replace(/^@+/, "").replace(/\s+/g, " ").toLocaleLowerCase();
  var parseBlocklistEntries = (text) => {
    const entries = /* @__PURE__ */ new Set();
    for (const part of text.split(/[,\n]/)) {
      const value = normalizeBlocklistValue(part);
      if (value) entries.add(value);
    }
    return [...entries];
  };
  var blocklistFromSettings = (channels, keywords) => ({
    channels: parseBlocklistEntries(channels),
    keywords: parseBlocklistEntries(keywords)
  });
  var blocklistChannelMatches = (text, channels) => {
    if (!text) return false;
    const normalized = normalizeBlocklistValue(text);
    return Boolean(normalized) && channels.some((value) => normalized === value);
  };
  var blocklistChannelsMatch = (texts, channels) => texts.some((text) => blocklistChannelMatches(text, channels));
  var blocklistTextMatches = (text, keywords) => {
    if (!text) return false;
    const normalized = normalizeBlocklistValue(text);
    return Boolean(normalized) && keywords.some((value) => normalized.includes(value));
  };
  var blocklistTextsMatch = (texts, keywords) => texts.some((text) => blocklistTextMatches(text, keywords));

  // src/blocklist.ts
  var blockedClass = "_nou_blocked";
  var itemSelector = [
    "ytm-media-item",
    "ytm-compact-video-renderer",
    "ytm-video-with-context-renderer",
    "ytd-video-renderer",
    "ytd-compact-video-renderer",
    "ytd-grid-video-renderer",
    "ytd-rich-item-renderer",
    "yt-lockup-view-model",
    "yt-lockup-metadata-view-model"
  ].join(",");
  var getText = (element) => element?.textContent?.trim() || "";
  function getTitleTexts(item) {
    return [
      getText(item.querySelector("h3")),
      getText(item.querySelector("h4")),
      getText(item.querySelector("#video-title")),
      getText(item.querySelector("a#video-title")),
      getText(item.querySelector(".yt-lockup-metadata-view-model-wiz__title")),
      getText(item.querySelector('.yt-core-attributed-string[role="text"]'))
    ].filter(Boolean);
  }
  function getChannelTexts(item) {
    const channelLinks = Array.from(
      item.querySelectorAll(
        'a[href^="/@"],a[href^="/channel/"],a[href^="/c/"],a[href^="/user/"],a[href*="youtube.com/@"],a[href*="youtube.com/channel/"]'
      )
    );
    return [
      ...channelLinks.flatMap((link) => [
        link.textContent?.trim(),
        link.getAttribute("href") || "",
        link.href || ""
      ]),
      getText(item.querySelector(".yt-lockup-metadata-view-model-wiz__metadata")),
      getText(item.querySelector("ytm-badge-and-byline-renderer")),
      getText(item.querySelector("ytd-channel-name")),
      getText(item.querySelector("#channel-name"))
    ].filter(Boolean);
  }
  function installBlocklistFilter(blocklist) {
    addStyle("_nou_blocklist_css", `.${blockedClass}{display:none!important;}`);
    const itemMatches = (item) => blocklistTextsMatch(getTitleTexts(item), blocklist.keywords) || blocklistChannelsMatch(getChannelTexts(item), blocklist.channels) || blocklistTextMatches(item.getAttribute("aria-label") || "", blocklist.keywords);
    const apply = (item) => item.classList.toggle(blockedClass, itemMatches(item));
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

  // src/clickbait.ts
  var TARGETS = {
    "Frame from the start": "hq1",
    "Frame from the middle": "hq2",
    "Frame from the end": "hq3"
  };
  var thumbnailTarget = (choice) => TARGETS[choice];
  var QUALITIES = "default|mqdefault|hqdefault|sddefault|hq720|hq1|hq2|hq3|maxresdefault";
  var THUMB_RE = new RegExp(
    `^(https?://i\\d?\\.ytimg\\.com/(?:vi|vi_webp)/[^/]+/)(${QUALITIES})(_custom_\\d+)?(\\.[a-zA-Z0-9]+)(.*)$`
  );
  var THUMB_IN_STYLE_RE = new RegExp(
    `(https?://i\\d?\\.ytimg\\.com/(?:vi|vi_webp)/[^/]+/)(${QUALITIES})(_custom_\\d+)?(\\.[a-zA-Z0-9]+)`,
    "g"
  );
  function transformUrl(url, target) {
    const match = THUMB_RE.exec(url);
    if (!match || match[2] === target) return url;
    return `${match[1]}${target}${match[4]}${match[5]}`;
  }
  var transformStyle = (value, target) => value.replace(THUMB_IN_STYLE_RE, `$1${target}$4`);
  function installClickbaitThumbnails(target) {
    try {
      const descriptor = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, "src");
      const setSource = descriptor?.set;
      const getSource = descriptor?.get;
      if (setSource && getSource) {
        Object.defineProperty(HTMLImageElement.prototype, "src", {
          configurable: true,
          enumerable: true,
          get() {
            return getSource.call(this);
          },
          set(value) {
            setSource.call(this, typeof value === "string" ? transformUrl(value, target) : value);
          }
        });
      }
    } catch (error) {
      console.warn("[YouTube enhancements] thumbnails:", error);
    }
    try {
      const setAttribute = Element.prototype.setAttribute;
      Element.prototype.setAttribute = function(name, value) {
        if (this instanceof HTMLImageElement && name === "src" && typeof value === "string") {
          return setAttribute.call(this, name, transformUrl(value, target));
        }
        return setAttribute.call(this, name, value);
      };
    } catch (error) {
      console.warn("[YouTube enhancements] thumbnails:", error);
    }
    const sweep = () => {
      document.querySelectorAll("img").forEach((image) => {
        const current = image.getAttribute("src");
        if (!current) return;
        const next = transformUrl(current, target);
        if (next !== current) image.setAttribute("src", next);
      });
      document.querySelectorAll('[style*="ytimg.com"]').forEach((element) => {
        const style = element.getAttribute("style");
        if (!style) return;
        const next = transformStyle(style, target);
        if (next !== style) element.setAttribute("style", next);
      });
    };
    if (document.body) sweep();
    document.addEventListener("DOMContentLoaded", sweep);
    setTimeout(sweep, 1e3);
    setTimeout(sweep, 3e3);
  }

  // src/lib/user-styles.ts
  var builtinStyles = [
    {
      setting: "hideMixes",
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
    `
    },
    {
      setting: "hideCommunityPosts",
      css: `
      ytd-rich-section-renderer:has(ytd-post-renderer),
      ytd-post-renderer,
      ytd-backstage-post-thread-renderer,
      ytm-rich-section-renderer:has(ytm-post-renderer),
      ytm-post-renderer,
      ytm-backstage-post-thread-renderer {
        display: none !important;
      }
    `
    },
    {
      setting: "hideAskButton",
      css: `
      .you-chat-entrypoint-button,
      yt-button-view-model:has(> .you-chat-entrypoint-button) {
        display: none !important;
      }
    `
    },
    {
      setting: "hideHomeFeed",
      // m.youtube.com has no page-subtype attribute; its home tab is told by its browse endpoint
      // (FEwhat_to_watch) on the tab's wrapper.
      css: `
      ytd-browse[page-subtype='home'] #contents,
      ytm-browse [tab-identifier='FEwhat_to_watch'] ytm-rich-grid-renderer {
        display: none !important;
      }
    `
    },
    {
      setting: "hideRelatedVideos",
      css: `
      #related,
      ytd-watch-next-secondary-results-renderer,
      ytm-watch-next-secondary-results-renderer,
      ytm-item-section-renderer[section-identifier='related-items'] {
        display: none !important;
      }
    `
    },
    {
      setting: "hideEndScreens",
      css: `
      .ytp-ce-element,
      .ytp-endscreen-content,
      .ytp-suggested-video-overlay,
      .ytp-pause-overlay,
      .ytp-pause-overlay-container {
        display: none !important;
      }
    `
    }
  ];
  var shortsNavigationCss = `
  yt-tab-shape[tab-title='Shorts'],
  ytd-guide-entry-renderer:has(a[href^='/shorts']),
  ytd-mini-guide-entry-renderer:has(a[href^='/shorts']),
  ytm-pivot-bar-item-renderer:has(.pivot-shorts) {
    display: none !important;
  }
`;

  // src/css.ts
  var adsCss = `
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
  var appLinksCss = `
  a.app-install-link,
  a[href^='intent:'],
  a[href^='vnd.youtube:'] {
    display: none !important;
  }
`;
  var shortsCss = `
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
  var sponsorSegmentsCss = `
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
  var mobileProgressCss = `
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
  var musicMobileCss = `
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
  var liveChatCss = `
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
  function buildCss() {
    const parts = [];
    if (flag("blockAds", true)) parts.push(adsCss, appLinksCss);
    if (flag("hideShorts", true) && !isMusic) parts.push(shortsCss, shortsNavigationCss);
    if (flag("sponsorBlock", true)) {
      parts.push(sponsorSegmentsCss);
      if (isMobileSite) parts.push(mobileProgressCss);
    }
    if (isMusic && isMobileAgent) parts.push(musicMobileCss);
    if (isMobileSite && flag("liveChat", true)) parts.push(liveChatCss);
    if (!isMusic) {
      for (const style of builtinStyles) {
        if (flag(style.setting, false)) parts.push(style.css);
      }
    }
    return parts.join("\n");
  }
  function injectCss() {
    const css = buildCss();
    if (css) addStyle("_nou_injected_css", css);
  }

  // src/dialogs.ts
  var handled2 = false;
  function watchDialog() {
    if (handled2) return;
    const target = document.querySelector("tp-yt-paper-dialog");
    if (!target) return;
    handled2 = true;
    new MutationObserver(() => {
      const dismiss = target.querySelector(
        "ytmusic-mealbar-promo-renderer .dismiss-button"
      );
      dismiss?.click();
    }).observe(target, { attributes: true });
  }
  function installPromoDismissal() {
    whenRoot((root) => {
      const observer = new MutationObserver(() => {
        watchDialog();
        if (handled2) observer.disconnect();
      });
      observer.observe(root, { childList: true, subtree: true });
      watchDialog();
      if (handled2) observer.disconnect();
    });
  }

  // src/dislikes.ts
  var countClass = "_nou_dislike_count";
  var loadedClass = "_nou_dislike_loaded";
  var styleId = "_nou_dislike_css";
  var apiUrl = "https://returnyoutubedislikeapi.com/votes?videoId=";
  var cache = /* @__PURE__ */ new Map();
  var pending = /* @__PURE__ */ new Map();
  var currentVideoId = "";
  var requestId = 0;
  var dislikeContainerSelector = ["dislike-button-view-model", "#segmented-dislike-button"].join(
    ","
  );
  var dislikeButtonSelector = [
    "dislike-button-view-model button",
    "#segmented-dislike-button button"
  ].join(",");
  var labelSelector = [
    ".ytSpecButtonShapeNextButtonTextContent",
    ".ytSpecButtonShapeNextButtonTextContent span",
    ".yt-core-attributed-string",
    ".button-renderer-text"
  ].join(",");
  function getVideoId() {
    const url = new URL(location.href);
    if (url.pathname === "/shorts") return "";
    if (url.pathname.startsWith("/shorts/")) return url.pathname.split("/")[2] || "";
    return url.searchParams.get("v") || "";
  }
  function formatCount(value) {
    try {
      return new Intl.NumberFormat(void 0, {
        notation: "compact",
        maximumFractionDigits: 1
      }).format(value);
    } catch {
      return String(value);
    }
  }
  function ensureStyle() {
    if (document.getElementById(styleId)) return;
    const style = document.createElement("style");
    style.id = styleId;
    style.textContent = `
    .${countClass} {
      display: inline-flex !important;
      align-items: center !important;
      margin-left: 6px !important;
      font: inherit !important;
      font-size: 0.8em !important;
      font-weight: 400 !important;
      white-space: nowrap !important;
    }
  `;
    (document.head || document.documentElement).appendChild(style);
  }
  function removeCounts() {
    document.querySelectorAll(`.${countClass}`).forEach((node) => {
      const button = node.closest("button");
      if (button?.dataset.nouDislikeIconButton === "true") {
        button.classList.add("ytSpecButtonShapeNextIconButton");
        button.classList.remove("ytSpecButtonShapeNextIconLeading");
        delete button.dataset.nouDislikeIconButton;
      }
      const originalText = node.dataset.nouOriginalText;
      if (originalText != null && node.dataset.nouReused === "true") {
        node.textContent = originalText;
        node.classList.remove(countClass);
        delete node.dataset.nouOriginalText;
        delete node.dataset.nouReused;
        return;
      }
      node.remove();
    });
    document.querySelectorAll(`.${loadedClass}`).forEach((node) => node.classList.remove(loadedClass));
  }
  function removeStaleCounts(buttons) {
    const activeButtons = new Set(buttons);
    document.querySelectorAll(`.${countClass}`).forEach((node) => {
      const button = node.closest("button");
      if (!button || !activeButtons.has(button)) node.remove();
    });
  }
  function isVisible(element) {
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }
  function getDislikeButtons() {
    const buttons = [
      ...Array.from(document.querySelectorAll(dislikeButtonSelector)),
      ...Array.from(
        document.querySelectorAll(
          [
            'button[aria-label*="Dislike"]',
            'button[aria-label*="dislike"]',
            '[role="button"][aria-label*="Dislike"]',
            '[role="button"][aria-label*="dislike"]'
          ].join(",")
        )
      )
    ];
    return [...new Set(buttons)].filter((button) => {
      if (!isVisible(button)) return false;
      if (button.closest(dislikeContainerSelector)) return true;
      const label = `${button.getAttribute("aria-label") || ""} ${button.getAttribute("title") || ""}`;
      if (!/dislike/i.test(label)) return false;
      return Boolean(
        button.closest(
          [
            "ytd-segmented-like-dislike-button-renderer",
            "segmented-like-dislike-button-view-model",
            "ytm-slim-video-action-bar-renderer",
            "ytm-like-button-renderer"
          ].join(",")
        )
      );
    });
  }
  function renderCount(count) {
    ensureStyle();
    const buttons = getDislikeButtons();
    removeStaleCounts(buttons);
    for (const button of buttons) {
      let node = button.querySelector(`.${countClass}`);
      if (node && !node.classList.contains("ytSpecButtonShapeNextButtonTextContent")) {
        node.remove();
        node = null;
      }
      if (!node) {
        node = Array.from(button.querySelectorAll(labelSelector)).find(isVisible) || null;
        if (node) {
          node.dataset.nouOriginalText = node.textContent || "";
          node.dataset.nouReused = "true";
          node.classList.add(countClass);
        } else {
          node = document.createElement("div");
          node.className = `ytSpecButtonShapeNextButtonTextContent ${countClass}`;
          const feedback = button.querySelector("yt-touch-feedback-shape");
          button.insertBefore(node, feedback || null);
          if (button.classList.contains("ytSpecButtonShapeNextIconButton")) {
            button.dataset.nouDislikeIconButton = "true";
            button.classList.remove("ytSpecButtonShapeNextIconButton");
            button.classList.add("ytSpecButtonShapeNextIconLeading");
          }
        }
      }
      if (node.textContent !== count) node.textContent = count;
      button.classList.add(loadedClass);
    }
  }
  async function fetchCount(videoId2) {
    const cached = cache.get(videoId2);
    if (cached) return cached;
    const existing = pending.get(videoId2);
    if (existing) return existing;
    const promise = pageFetch(`${apiUrl}${encodeURIComponent(videoId2)}`).then(async (response) => {
      if (!response.ok) throw new Error(`Return YouTube Dislike answered ${response.status}`);
      const data = await response.json();
      const dislikes = typeof data === "object" && data !== null ? data.dislikes : void 0;
      const count = typeof dislikes === "number" ? formatCount(dislikes) : "";
      if (count) cache.set(videoId2, count);
      return count;
    }).finally(() => pending.delete(videoId2));
    pending.set(videoId2, promise);
    return promise;
  }
  async function updateDislikes() {
    const videoId2 = getVideoId();
    if (!videoId2) {
      currentVideoId = "";
      removeCounts();
      return;
    }
    const id = ++requestId;
    if (videoId2 !== currentVideoId) {
      currentVideoId = videoId2;
      removeCounts();
    }
    try {
      const count = await fetchCount(videoId2);
      if (id === requestId && videoId2 === currentVideoId && count) renderCount(count);
    } catch (error) {
      console.warn("[YouTube enhancements] dislikes:", error);
    }
  }
  function installDislikeCount() {
    onNavigate(() => void updateDislikes());
    const onMutations = coalesce(() => {
      const videoId2 = getVideoId();
      const cached = cache.get(videoId2);
      if (videoId2 && videoId2 === currentVideoId && cached) {
        renderCount(cached);
        return;
      }
      void updateDislikes();
    }, 250);
    whenRoot((root) => {
      void updateDislikes();
      new MutationObserver(onMutations).observe(root, { childList: true, subtree: true });
    });
  }

  // src/h264ify.ts
  var DISALLOWED = ["webm", "vp8", "vp9", "av01"];
  var allowed = (type) => typeof type === "string" && !DISALLOWED.some((codec) => type.includes(codec));
  function installH264ify() {
    try {
      const canPlayType = HTMLMediaElement.prototype.canPlayType;
      HTMLVideoElement.prototype.canPlayType = function(type) {
        return allowed(type) ? canPlayType.call(this, type) : "";
      };
    } catch (error) {
      console.warn("[YouTube enhancements] H.264:", error);
    }
    try {
      if (typeof MediaSource !== "undefined" && typeof MediaSource.isTypeSupported === "function") {
        const isTypeSupported = MediaSource.isTypeSupported.bind(MediaSource);
        MediaSource.isTypeSupported = (type) => allowed(type) && isTypeSupported(type);
      }
    } catch (error) {
      console.warn("[YouTube enhancements] H.264:", error);
    }
  }

  // src/lib/intercept.ts
  var AD_KEYS = ["adBreakHeartbeatParams", "adPlacements", "adSlots", "playerAds"];
  var RE_INTERCEPT = /^\/youtubei\/v1\/(browse|get_watch|next|player|reel|search)/;
  var blocksAds = (options) => options.blockAds !== false;
  var stripAdKeys = (data) => {
    if (!isObject(data)) return;
    for (const key of AD_KEYS) delete data[key];
  };
  function filterPlayerResponse(data, options = {}) {
    if (blocksAds(options)) stripAdKeys(data);
    applyPlayerResponseOriginalTitle(data, options);
    rewriteOriginalTitles(data, options);
    return data;
  }
  function transformPlayerResponse(text, _blocklist, options = {}) {
    return JSON.stringify(filterPlayerResponse(JSON.parse(text), options));
  }
  function transformGetWatchResponse(text, blocklist, options = {}) {
    const data = JSON.parse(text);
    for (const part of Array.isArray(data) ? data : [data]) {
      const playerResponse = pick(part, "playerResponse");
      if (blocksAds(options)) stripAdKeys(playerResponse);
      applyPlayerResponseOriginalTitle(playerResponse, options);
    }
    rewriteOriginalTitles(data, options);
    filterListResponse(data, blocklist, options);
    return JSON.stringify(data);
  }
  function transformSearchResponse(text, blocklist, options = {}) {
    const data = JSON.parse(text);
    const hideShorts = options.hideShorts !== false;
    rewriteOriginalTitles(data, options);
    const sectionListRenderer = (
      // mobile
      pick(data, "contents", "sectionListRenderer") || // desktop
      pick(
        data,
        "contents",
        "twoColumnSearchResultsRenderer",
        "primaryContents",
        "sectionListRenderer"
      )
    );
    const itemSectionRenderer = pick(sectionListRenderer, "contents", 0, "itemSectionRenderer") || pick(
      data,
      "onResponseReceivedCommands",
      0,
      "appendContinuationItemsAction",
      "continuationItems",
      0,
      "itemSectionRenderer"
    );
    if (isObject(itemSectionRenderer) && Array.isArray(itemSectionRenderer.contents)) {
      itemSectionRenderer.contents = itemSectionRenderer.contents.filter(
        (item) => !(hideShorts && isShortsItem(item)) && !itemMatchesBlocklist(item, blocklist)
      );
    }
    filterListResponse(data, blocklist, { ...options, hideShorts });
    return JSON.stringify(data);
  }
  function transformBrowseResponse(text, blocklist, options = {}) {
    const data = JSON.parse(text);
    rewriteOriginalTitles(data, options);
    filterListResponse(data, blocklist, options);
    return JSON.stringify(data);
  }
  function transformReelResponse(text, blocklist, options = {}) {
    const data = JSON.parse(text);
    if (blocksAds(options)) stripAdKeys(pick(data, "playerResponse"));
    rewriteOriginalTitles(data, options);
    filterListResponse(data, blocklist, options);
    return JSON.stringify(data);
  }
  function filterListResponse(data, blocklist, options = {}) {
    if (!isObject(data)) return;
    const dropAds = blocksAds(options);
    for (const key of Object.keys(data)) {
      const value = data[key];
      if (Array.isArray(value)) {
        const kept = value.filter(
          (item) => !(dropAds && isAdItem(item)) && !(options.hideShorts && isShortsItem(item)) && !itemMatchesBlocklist(item, blocklist)
        );
        data[key] = kept;
        kept.forEach((item) => filterListResponse(item, blocklist, options));
      } else if (isObject(value)) {
        filterListResponse(value, blocklist, options);
      }
    }
  }
  var SHORTS_PATH = /^\/shorts(?:\/|\?|$)/;
  function isShortsItem(item) {
    if (!isObject(item)) return false;
    const candidates = [
      item,
      pick(item, "richItemRenderer", "content"),
      pick(item, "richSectionRenderer", "content")
    ].filter(isObject);
    return candidates.some((card) => {
      if (card.reelShelfRenderer || card.reelItemRenderer || card.shortsLockupViewModel || card.shortsShelfViewModel) {
        return true;
      }
      if (pick(card, "richShelfRenderer", "isShorts")) return true;
      if (pickArray(card, "gridShelfViewModel", "contents").some(
        (child) => Boolean(pick(child, "shortsLockupViewModel"))
      )) {
        return true;
      }
      const renderer = card.videoWithContextRenderer || card.videoRenderer || card.compactVideoRenderer || card.gridVideoRenderer || card.lockupViewModel;
      const endpoint = pick(renderer, "navigationEndpoint") || pick(renderer, "onTap", "innertubeCommand") || pick(renderer, "rendererContext", "commandContext", "onTap", "innertubeCommand");
      const url = pick(endpoint, "commandMetadata", "webCommandMetadata", "url");
      return Boolean(
        pick(endpoint, "reelWatchEndpoint") || typeof url === "string" && SHORTS_PATH.test(url)
      );
    });
  }
  var AD_RENDERER_KEYS = [
    "adSlotRenderer",
    "adsEngagementPanelRenderer",
    "bannerPromoRenderer",
    "brandVideoShelfRenderer",
    "brandVideoSingletonRenderer",
    "carouselAdRenderer",
    "compactPromotedItemRenderer",
    "compactPromotedVideoRenderer",
    "displayAdRenderer",
    "inFeedAdLayoutRenderer",
    "mealbarPromoRenderer",
    "primetimePromoRenderer",
    "promotedSparklesTextSearchRenderer",
    "promotedSparklesWebRenderer",
    "promotedVideoRenderer",
    "searchPyvRenderer",
    "statementBannerRenderer"
  ];
  function isAdItem(item) {
    if (!isObject(item)) return false;
    if (pick(item, "command", "reelWatchEndpoint", "adClientParams", "isAd")) return true;
    const sectionContents = pickArray(item, "itemSectionRenderer", "contents");
    const candidates = [
      item,
      pick(item, "richItemRenderer", "content"),
      pick(item, "richSectionRenderer", "content"),
      sectionContents.length === 1 ? sectionContents[0] : void 0
    ].filter(isObject);
    return candidates.some((candidate) => AD_RENDERER_KEYS.some((key) => candidate[key]));
  }
  function itemMatchesBlocklist(item, blocklist) {
    if (!blocklist || isBlocklistEmpty(blocklist)) return false;
    return getDirectRenderers(item).some((renderer) => rendererMatchesBlocklist(renderer, blocklist));
  }
  function getDirectRenderers(item) {
    if (!isObject(item)) return [];
    return [
      item.videoRenderer,
      item.compactVideoRenderer,
      item.gridVideoRenderer,
      item.videoWithContextRenderer,
      item.reelItemRenderer,
      item.lockupViewModel,
      pick(item, "richItemRenderer", "content", "videoRenderer"),
      pick(item, "richItemRenderer", "content", "lockupViewModel")
    ].filter(isObject);
  }
  function rendererMatchesBlocklist(renderer, blocklist) {
    return blocklistTextsMatch(extractTitleTexts(renderer), blocklist.keywords) || blocklistChannelsMatch(extractChannelTexts(renderer), blocklist.channels);
  }
  function extractTitleTexts(renderer) {
    return [
      textOf(renderer.title),
      textOf(renderer.headline),
      textOf(pick(renderer, "metadata", "lockupMetadataViewModel", "title"))
    ].filter(Boolean);
  }
  function findBrowseEndpoints(node, results = { ids: /* @__PURE__ */ new Set(), urls: /* @__PURE__ */ new Set(), names: /* @__PURE__ */ new Set() }) {
    if (!isObject(node)) return results;
    const browseEndpoint = node.browseEndpoint;
    if (isObject(browseEndpoint)) {
      const { browseId, canonicalBaseUrl } = browseEndpoint;
      if (typeof browseId === "string" && browseId) results.ids.add(browseId);
      if (typeof canonicalBaseUrl === "string" && canonicalBaseUrl)
        results.urls.add(canonicalBaseUrl);
    }
    if (typeof node.text === "string" && node.text && pick(node, "navigationEndpoint", "browseEndpoint")) {
      results.names.add(node.text);
    }
    for (const key of Object.keys(node)) findBrowseEndpoints(node[key], results);
    return results;
  }
  function extractChannelTexts(renderer) {
    const endpoints = findBrowseEndpoints(renderer);
    const texts = [...endpoints.names, ...endpoints.urls, ...endpoints.ids];
    if (texts.length) return texts;
    const run = (field, key) => pick(renderer, field, "runs", 0, "navigationEndpoint", "browseEndpoint", key);
    return [
      textOf(renderer.ownerText),
      textOf(renderer.shortBylineText),
      textOf(renderer.longBylineText),
      textOf(pick(renderer, "owner", "videoOwnerRenderer", "title")),
      run("shortBylineText", "canonicalBaseUrl"),
      run("shortBylineText", "browseId"),
      run("ownerText", "canonicalBaseUrl"),
      run("ownerText", "browseId"),
      textOf(pick(renderer, "metadata", "lockupMetadataViewModel", "metadata")),
      pickArray(renderer, "contentMetadata", "runs").map((part) => textOf(pick(part, "text"))).join(" ")
    ].filter((text) => typeof text === "string" && text !== "");
  }
  function applyPlayerResponseOriginalTitle(data, options) {
    if (!options.showOriginalTitles) return;
    const originalTitle = textOf(pick(data, "videoDetails", "title"));
    if (!originalTitle) return;
    setTextNode(pick(data, "microformat", "playerMicroformatRenderer", "title"), originalTitle);
    setTextNode(pick(data, "videoDetails", "title"), originalTitle);
  }
  function rewriteOriginalTitles(data, options) {
    if (!options.showOriginalTitles || !isObject(data)) return;
    walk(data, (node) => {
      for (const renderer of getDirectRenderers(node)) rewriteRendererTitle(renderer);
      rewriteRendererTitle(node);
    });
  }
  function walk(node, visit) {
    if (!isObject(node)) return;
    visit(node);
    for (const key of Object.keys(node)) walk(node[key], visit);
  }
  function rewriteRendererTitle(renderer) {
    const originalTitle = getOriginalTitle(renderer);
    if (!originalTitle) return;
    setTextNode(renderer.title, originalTitle);
    setTextNode(renderer.headline, originalTitle);
    setTextNode(pick(renderer, "metadata", "lockupMetadataViewModel", "title"), originalTitle);
  }
  function getOriginalTitle(renderer) {
    return [
      renderer.originalTitle,
      renderer.untranslatedTitle,
      renderer.untranslatedVideoTitle,
      pick(renderer, "title", "originalTitle"),
      pick(renderer, "title", "untranslatedTitle"),
      pick(renderer, "headline", "originalTitle"),
      pick(renderer, "metadata", "lockupMetadataViewModel", "originalTitle"),
      pick(renderer, "metadata", "lockupMetadataViewModel", "title", "originalTitle")
    ].map(textOf).find(Boolean) || "";
  }
  function setTextNode(node, text) {
    if (!isObject(node)) return;
    if (typeof node.simpleText === "string") node.simpleText = text;
    if (typeof node.content === "string") node.content = text;
    if (Array.isArray(node.runs) && node.runs.length) {
      node.runs.forEach((run, index) => {
        if (isObject(run) && typeof run.text === "string") run.text = index === 0 ? text : "";
      });
    }
  }

  // src/intercept.ts
  var TRANSFORMS = {
    browse: transformBrowseResponse,
    get_watch: transformGetWatchResponse,
    next: transformBrowseResponse,
    player: transformPlayerResponse,
    reel: transformReelResponse,
    search: transformSearchResponse
  };
  var report2 = (where, error) => console.error(`[YouTube enhancements] ${where}:`, error);
  var endpointOf = (address) => {
    try {
      return new URL(address, location.origin).pathname.match(RE_INTERCEPT)?.[1];
    } catch {
      return void 0;
    }
  };
  function filterGlobal(name, filter) {
    const run = (value) => {
      try {
        return filter(value);
      } catch (error) {
        report2(name, error);
        return value;
      }
    };
    let current = run(window[name]);
    Object.defineProperty(window, name, {
      get: () => current,
      set: (value) => {
        current = run(value);
      },
      configurable: true
    });
  }
  function installIntercept(options, blocklist) {
    filterGlobal("ytInitialData", (value) => {
      filterListResponse(value, blocklist, options);
      return value;
    });
    filterGlobal("ytInitialPlayerResponse", (value) => filterPlayerResponse(value, options));
    filterGlobal("ytInitialReelWatchSequenceResponse", (value) => {
      const reelOptions = { blockAds: options.blockAds };
      if (typeof value === "string") return transformReelResponse(value, blocklist, reelOptions);
      filterListResponse(value, blocklist, reelOptions);
      return value;
    });
    const pageFetch2 = window.fetch;
    window.fetch = async (input, init) => {
      const response = await pageFetch2.call(window, input, init);
      const address = input instanceof Request ? input.url : input.toString();
      const transform = TRANSFORMS[endpointOf(address) ?? ""];
      if (!transform || response.status !== 200) return response;
      const text = await response.text();
      const responseInit = {
        status: response.status,
        statusText: response.statusText,
        headers: response.headers
      };
      try {
        return new Response(transform(text, blocklist, options), responseInit);
      } catch (error) {
        report2("fetch", error);
      }
      return new Response(text, responseInit);
    };
    const responseGetter = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, "response")?.get;
    const textGetter = Object.getOwnPropertyDescriptor(XMLHttpRequest.prototype, "responseText")?.get;
    if (!responseGetter || !textGetter) return;
    const xhrOpen = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function(method, url, ...rest) {
      const transform = TRANSFORMS[endpointOf(url.toString()) ?? ""];
      let cached = false;
      let filteredText;
      let filteredJson;
      const read = (asText) => {
        const original = () => (asText ? textGetter : responseGetter).call(this);
        if (!transform || this.readyState !== 4 || this.status !== 200 || this.responseType !== "" && this.responseType !== "text" && this.responseType !== "json") {
          return original();
        }
        if (asText && this.responseType === "json") return original();
        const isJson = this.responseType === "json";
        if (cached) return isJson ? filteredJson : filteredText;
        cached = true;
        try {
          const text = isJson ? JSON.stringify(responseGetter.call(this)) : textGetter.call(this);
          filteredText = transform(String(text), blocklist, options);
          if (isJson) filteredJson = JSON.parse(String(filteredText));
        } catch (error) {
          report2("XMLHttpRequest", error);
          if (isJson) filteredJson = responseGetter.call(this);
          else filteredText = textGetter.call(this);
        }
        return isJson ? filteredJson : filteredText;
      };
      Object.defineProperties(this, {
        response: { configurable: true, get: () => read(false) },
        responseText: { configurable: true, get: () => read(true) }
      });
      return Reflect.apply(xhrOpen, this, [method, url, ...rest]);
    };
  }

  // src/lib/live-chat-replay.ts
  var messageRenderers = [
    "liveChatTextMessageRenderer",
    "liveChatPaidMessageRenderer",
    "liveChatPaidStickerRenderer",
    "liveChatMembershipItemRenderer",
    "liveChatViewerEngagementMessageRenderer",
    "liveChatSponsorshipsGiftRedemptionAnnouncementRenderer"
  ];
  function chatReplayImageUrl(value) {
    if (typeof value !== "string") return void 0;
    const url = value.startsWith("//") ? `https:${value}` : value;
    return url.startsWith("https://") ? url : void 0;
  }
  function chatReplayPollDelay(actions = []) {
    return actions.some((action) => pickArray(action, "replayChatItemAction", "actions").length) ? 1e3 : 5e3;
  }
  function shouldPrefetchChatReplay(time, bufferedUntil, rate, thresholdMsec) {
    return bufferedUntil - time <= Math.max(thresholdMsec / 1e3, rate * 2);
  }
  var asMessage = (item) => {
    for (const renderer of messageRenderers) {
      const message = pick(item, renderer);
      if (isObject(message) && typeof message.id === "string" && message.id) {
        return message;
      }
    }
    return void 0;
  };
  var ChatReplayBuffer = class {
    pending = [];
    messages = /* @__PURE__ */ new Map();
    bufferedUntil = 0;
    clear() {
      this.pending = [];
      this.messages.clear();
      this.bufferedUntil = 0;
    }
    append(actions = []) {
      for (const action of actions) {
        const replay = pick(action, "replayChatItemAction");
        if (!isObject(replay)) continue;
        const time = Number(replay.videoOffsetTimeMsec) / 1e3;
        if (!Number.isFinite(time)) continue;
        this.bufferedUntil = Math.max(this.bufferedUntil, time);
        this.pending.push({ time, actions: pickArray(replay, "actions") });
      }
      this.pending.sort((first, second) => first.time - second.time);
    }
    advance(time) {
      let changed = false;
      for (let next = this.pending[0]; next && next.time <= time; next = this.pending[0]) {
        this.pending.shift();
        for (const action of next.actions) {
          const item = pick(action, "addChatItemAction", "item") || pick(action, "replaceChatItemAction", "replacementItem");
          if (item) {
            const message = asMessage(item);
            const replacedId = pickString(action, "replaceChatItemAction", "targetItemId");
            if (message) {
              if (replacedId && this.messages.has(replacedId) && replacedId !== message.id) {
                const entries = Array.from(
                  this.messages,
                  ([id, value]) => id === replacedId ? [message.id, message] : [id, value]
                );
                this.messages.clear();
                for (const [id, value] of entries) this.messages.set(id, value);
              } else {
                this.messages.set(message.id, message);
              }
              changed = true;
            } else if (replacedId) {
              changed = this.messages.delete(replacedId) || changed;
            }
          }
          const removedId = pickString(action, "markChatItemAsDeletedAction", "targetItemId") || pickString(action, "removeChatItemAction", "targetItemId");
          if (removedId) changed = this.messages.delete(removedId) || changed;
          const authorId = pickString(action, "markChatItemsByAuthorAsDeletedAction", "externalChannelId") || pickString(action, "removeChatItemByAuthorAction", "externalChannelId");
          if (authorId) {
            for (const [id, message] of this.messages) {
              if (message.authorExternalChannelId === authorId) {
                changed = this.messages.delete(id) || changed;
              }
            }
          }
        }
        while (this.messages.size > 100) {
          const oldest = this.messages.keys().next().value;
          if (oldest === void 0) break;
          this.messages.delete(oldest);
        }
      }
      return changed;
    }
  };
  function getChatReplay(response) {
    const chat = pick(
      response,
      "contents",
      "twoColumnWatchNextResults",
      "conversationBar",
      "liveChatRenderer"
    );
    const continuation = pickString(
      chat,
      "continuations",
      0,
      "reloadContinuationData",
      "continuation"
    );
    return pick(chat, "isReplay") ? continuation : void 0;
  }

  // src/lib/youtube-auth.ts
  async function getYouTubeAuthorization(origin, cookie, now = Date.now()) {
    const cookies = new Map(
      cookie.split(";").map((part) => {
        const separator = part.indexOf("=");
        return [part.slice(0, separator).trim(), part.slice(separator + 1)];
      })
    );
    const thirdParty = cookies.get("__Secure-3PAPISID");
    const sources = [
      ["SAPISIDHASH", cookies.get("SAPISID") || thirdParty],
      ["SAPISID1PHASH", cookies.get("__Secure-1PAPISID")],
      ["SAPISID3PHASH", thirdParty]
    ];
    const timestamp = Math.floor(now / 1e3);
    const signatures = await Promise.all(
      sources.filter((source) => Boolean(source[1])).map(async ([scheme, sid]) => {
        const digest = await crypto.subtle.digest(
          "SHA-1",
          new TextEncoder().encode(`${timestamp} ${sid} ${origin}`)
        );
        const hash = Array.from(
          new Uint8Array(digest),
          (byte) => byte.toString(16).padStart(2, "0")
        ).join("");
        return `${scheme} ${timestamp}_${hash}`;
      })
    );
    return signatures.length ? signatures.join(" ") : void 0;
  }

  // src/livechat-replay.ts
  async function request(path, data, signal) {
    const context = ytcfgValue("INNERTUBE_CONTEXT");
    const client = pick(context, "client");
    if (!isObject(context) || !isObject(client)) throw new Error("YouTube context is not ready");
    const headers = { "Content-Type": "application/json" };
    const authorization = ytcfgValue("LOGGED_IN") === false ? void 0 : await getYouTubeAuthorization(location.origin, document.cookie);
    if (authorization) {
      headers.Authorization = authorization;
      headers["X-Goog-AuthUser"] = String(ytcfgValue("SESSION_INDEX") ?? 0);
      headers["X-Origin"] = location.origin;
      const pageId = ytcfgValue("DELEGATED_SESSION_ID");
      if (typeof pageId === "string" && pageId) headers["X-Goog-PageId"] = pageId;
    }
    if (typeof client.visitorData === "string" && client.visitorData) {
      headers["X-Goog-Visitor-Id"] = client.visitorData;
    }
    const response = await pageFetch(`${location.origin}/youtubei/v1/${path}?prettyPrint=false`, {
      method: "POST",
      headers,
      signal,
      body: JSON.stringify({
        context: {
          ...context,
          client: {
            ...client,
            clientName: "WEB",
            platform: "DESKTOP",
            clientFormFactor: "UNKNOWN_FORM_FACTOR"
          }
        },
        ...data
      })
    });
    if (!response.ok) throw new Error(`Chat request failed: ${response.status}`);
    return response.json();
  }
  async function findChatReplay(videoId2, signal) {
    return getChatReplay(await request("next", { videoId: videoId2 }, signal));
  }
  function appendText(element, text) {
    const simpleText = pickString(text, "simpleText");
    if (simpleText) element.append(document.createTextNode(simpleText));
    for (const run of pickArray(text, "runs")) {
      const runText = pickString(run, "text");
      if (runText) element.append(document.createTextNode(runText));
      const emoji = pick(run, "emoji");
      if (!isObject(emoji)) continue;
      const image = document.createElement("img");
      image.className = "_nou_replay_emoji";
      image.alt = pickString(emoji, "image", "accessibility", "accessibilityData", "label") || pickString(emoji, "shortcuts", 0) || pickString(emoji, "emojiId") || "";
      const src = chatReplayImageUrl(pick(emoji, "image", "thumbnails", 0, "url"));
      if (src) {
        image.src = src;
        element.append(image);
      } else {
        element.append(document.createTextNode(image.alt));
      }
    }
  }
  function renderMessage(message) {
    const row = document.createElement("article");
    row.className = "_nou_replay_message";
    const photo = chatReplayImageUrl(pick(message.authorPhoto, "thumbnails", 0, "url"));
    if (photo) {
      const image = document.createElement("img");
      image.className = "_nou_replay_avatar";
      image.src = photo;
      image.alt = "";
      row.append(image);
    }
    const body = document.createElement("span");
    const timestamp = document.createElement("small");
    appendText(timestamp, message.timestampText);
    body.append(timestamp);
    const author = document.createElement("strong");
    appendText(author, message.authorName);
    body.append(author);
    appendText(body, message.purchaseAmountText);
    appendText(body, message.headerSubtext);
    appendText(body, message.message);
    const stickers = pickArray(message.sticker, "thumbnails");
    const sticker = chatReplayImageUrl(pick(stickers[stickers.length - 1], "url"));
    if (sticker) {
      const image = document.createElement("img");
      image.className = "_nou_replay_sticker";
      image.src = sticker;
      image.alt = pickString(message.sticker, "accessibility", "accessibilityData", "label") || "Paid sticker";
      body.append(image);
    }
    row.append(body);
    return row;
  }
  function mountChatReplay(container, videoId2, initialContinuation) {
    const panel = document.createElement("section");
    panel.className = "_nou_replay_panel";
    const header = document.createElement("header");
    const title = document.createElement("span");
    title.textContent = "Chat replay";
    header.append(title);
    const close = container.querySelector("._nou_livechat_close");
    if (close) header.append(close);
    const status = document.createElement("p");
    status.textContent = "Loading...";
    const retry = document.createElement("button");
    retry.className = "_nou_replay_retry";
    retry.textContent = "Retry";
    retry.hidden = true;
    const list = document.createElement("section");
    list.className = "_nou_replay_messages";
    list.setAttribute("aria-label", "Chat replay messages");
    panel.append(header, status, retry, list);
    container.prepend(panel);
    const buffer = new ChatReplayBuffer();
    let continuation = initialContinuation;
    let seekContinuation = initialContinuation;
    let controller = new AbortController();
    let revision = 0;
    let stopped = false;
    let loading = false;
    let failed = false;
    let lastTime;
    let prefetchThresholdMsec = 5e3;
    let nextRequestAt = 0;
    let hasLoaded = false;
    const rows = /* @__PURE__ */ new Map();
    function redraw2() {
      const atBottom = list.scrollHeight - list.scrollTop - list.clientHeight < 40;
      const listTop = list.getBoundingClientRect().top;
      const anchor = atBottom ? void 0 : Array.from(rows.entries()).find(
        ([id, row]) => buffer.messages.has(id) && row.element.getBoundingClientRect().bottom > listTop
      );
      const anchorTop = anchor?.[1].element.getBoundingClientRect().top;
      for (const [id, row] of rows) {
        if (!buffer.messages.has(id)) {
          row.element.remove();
          rows.delete(id);
        }
      }
      let previous;
      for (const [id, message] of buffer.messages) {
        let row = rows.get(id);
        if (!row || row.message !== message) {
          const element = renderMessage(message);
          row?.element.replaceWith(element);
          row = { message, element };
          rows.set(id, row);
        }
        const expected = previous ? previous.nextElementSibling : list.firstElementChild;
        if (row.element !== expected) list.insertBefore(row.element, expected);
        previous = row.element;
      }
      status.hidden = !failed && buffer.messages.size > 0;
      if (atBottom) {
        list.scrollTop = list.scrollHeight;
      } else if (anchor && anchorTop !== void 0) {
        const element = rows.get(anchor[0])?.element;
        if (element) list.scrollTop += element.getBoundingClientRect().top - anchorTop;
      }
    }
    function reset() {
      controller.abort();
      controller = new AbortController();
      revision++;
      loading = false;
      failed = false;
      nextRequestAt = 0;
      hasLoaded = false;
      continuation = seekContinuation;
      buffer.clear();
      list.replaceChildren();
      rows.clear();
      status.hidden = false;
      status.textContent = "Loading...";
      retry.hidden = true;
    }
    async function tick() {
      const player2 = getPlayer();
      if (stopped || !player2 || readVideo(player2)?.id !== videoId2) return;
      const time = player2.getCurrentTime?.();
      if (typeof time !== "number" || !Number.isFinite(time)) return;
      const rate = getVideoElement(player2)?.playbackRate || 1;
      if (lastTime !== void 0 && (time < lastTime - 1 || time > lastTime + Math.max(5, rate * 2))) {
        reset();
      }
      lastTime = time;
      if (buffer.advance(time)) redraw2();
      if (loading || failed || !continuation || performance.now() < nextRequestAt) return;
      if (hasLoaded && (player2.getPlayerState?.() !== PlayerState.playing || !shouldPrefetchChatReplay(time, buffer.bufferedUntil, rate, prefetchThresholdMsec))) {
        return;
      }
      loading = true;
      const requestRevision = revision;
      try {
        const response = await request(
          "live_chat/get_live_chat_replay",
          {
            continuation,
            currentPlayerState: { playerOffsetMs: String(Math.floor(time * 1e3)) }
          },
          controller.signal
        );
        if (stopped || requestRevision !== revision) return;
        const chat = pick(response, "continuationContents", "liveChatContinuation");
        if (!isObject(chat)) throw new Error("Chat replay is unavailable");
        const actions = pickArray(chat, "actions");
        buffer.append(actions);
        const continuations = pickArray(chat, "continuations");
        const next = pick(
          continuations.find((item) => pick(item, "liveChatReplayContinuationData")),
          "liveChatReplayContinuationData"
        );
        const seek = pick(
          continuations.find((item) => pick(item, "playerSeekContinuationData")),
          "playerSeekContinuationData"
        );
        continuation = pickString(next, "continuation") || "";
        seekContinuation = pickString(seek, "continuation") || initialContinuation;
        prefetchThresholdMsec = Number(pick(next, "timeUntilLastMessageMsec") ?? 5e3);
        nextRequestAt = performance.now() + chatReplayPollDelay(actions);
        hasLoaded = true;
        if (buffer.advance(player2.getCurrentTime?.() ?? time)) redraw2();
        status.hidden = buffer.messages.size > 0;
        status.textContent = "No messages at this point in the video.";
      } catch {
        if (stopped || requestRevision !== revision) return;
        failed = true;
        status.hidden = false;
        status.textContent = "Unable to load chat replay.";
        retry.hidden = false;
      } finally {
        if (requestRevision === revision) loading = false;
      }
    }
    retry.onclick = () => {
      reset();
      void tick();
    };
    const video = getVideoElement(getPlayer());
    const onSeek = () => {
      lastTime = void 0;
      reset();
      void tick();
    };
    video?.addEventListener("seeked", onSeek);
    const timer = setInterval(() => void tick(), 500);
    void tick();
    return () => {
      stopped = true;
      controller.abort();
      clearInterval(timer);
      video?.removeEventListener("seeked", onSeek);
    };
  }

  // src/livechat.ts
  var stopReplay;
  var replayLookup;
  var SVG = "http://www.w3.org/2000/svg";
  function icon(size, path, stroke) {
    const svg = document.createElementNS(SVG, "svg");
    svg.setAttribute("width", String(size));
    svg.setAttribute("height", String(size));
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("aria-hidden", "true");
    if (stroke) {
      svg.setAttribute("fill", "none");
      svg.setAttribute("stroke", "currentColor");
      svg.setAttribute("stroke-width", "2");
      svg.setAttribute("stroke-linecap", "round");
    } else {
      svg.setAttribute("fill", "currentColor");
    }
    const shape = document.createElementNS(SVG, "path");
    shape.setAttribute("d", path);
    svg.append(shape);
    return svg;
  }
  var crossIcon = () => icon(24, "m6 6 12 12M6 18 18 6", true);
  var chatIcon = () => icon(
    20,
    "M16 3v11H7.59L5 16.59V3h11m1-1H4v17l4-4h9V2zM8 18h8l4 4V6h-1v13.59L16.41 17H8v1z",
    false
  );
  function closeLiveChat() {
    stopReplay?.();
    stopReplay = void 0;
    document.querySelector("div#_nou_livechat")?.remove();
  }
  function showLiveChat(videoId2, replayContinuation) {
    closeLiveChat();
    const container = document.createElement("div");
    container.id = "_nou_livechat";
    const loading = document.createElement("div");
    loading.id = "_nou_livechat_loading";
    loading.textContent = "Loading...";
    const close = document.createElement("button");
    close.className = "_nou_livechat_close";
    close.setAttribute("aria-label", "Close chat");
    close.title = "Close chat";
    close.append(crossIcon());
    close.onclick = closeLiveChat;
    container.append(loading, close);
    document.body.append(container);
    if (replayContinuation) {
      loading.remove();
      stopReplay = mountChatReplay(container, videoId2, replayContinuation);
    } else {
      const frame = document.createElement("iframe");
      frame.src = `https://www.youtube.com/live_chat?v=${encodeURIComponent(videoId2)}&embed_domain=${location.hostname}`;
      loading.after(frame);
    }
    container.classList.toggle(
      "right",
      window.innerWidth > 1e3 && window.innerWidth > window.innerHeight
    );
  }
  function hideLiveChat() {
    replayLookup?.abort();
    replayLookup = void 0;
    closeLiveChat();
    document.querySelector("button#_nou_livechat_btn")?.remove();
  }
  function showLiveChatButton(videoId2, replayContinuation) {
    let button = document.querySelector("button#_nou_livechat_btn");
    const existed = Boolean(button);
    if (!button) {
      button = document.createElement("button");
      button.id = "_nou_livechat_btn";
    }
    button.replaceChildren(chatIcon(), replayContinuation ? "Chat replay" : "Live chat");
    button.onclick = () => showLiveChat(videoId2, replayContinuation);
    if (!existed) document.body.append(button);
  }
  async function showLiveChatReplayButton(videoId2) {
    replayLookup?.abort();
    const controller = new AbortController();
    replayLookup = controller;
    try {
      const continuation = await findChatReplay(videoId2, controller.signal);
      if (continuation && !controller.signal.aborted) showLiveChatButton(videoId2, continuation);
    } catch {
    }
  }
  var shownFor = "";
  function update(video, player2) {
    const id = video && playsMainVideo(player2) ? video.id : "";
    if (id === shownFor) return;
    shownFor = id;
    hideLiveChat();
    if (!video || !id) return;
    if (video.live) showLiveChatButton(id);
    else if (video.liveContent) void showLiveChatReplayButton(id);
  }
  function installLiveChat() {
    onVideoChange((video, player2) => whenBody(() => update(video, player2)));
    onNavigate(() => {
      const player2 = getPlayer();
      whenBody(() => update(readVideo(player2), player2));
    });
  }

  // src/offers.ts
  var LIST_ID = /^[A-Za-z0-9_-]+$/;
  var PERSONAL_LISTS = /* @__PURE__ */ new Set(["WL", "LL", "LM"]);
  var isPersonalList = (id) => PERSONAL_LISTS.has(id) || id.startsWith("RD");
  var SITE = isMusic ? "https://music.youtube.com" : "https://www.youtube.com";
  var loadedHref = location.href;
  var nonEmpty = (text) => text ? text.trim() || void 0 : void 0;
  var artistName = (author) => nonEmpty(author?.replace(/ - Topic$/, ""));
  function fromPlayer(player2, id) {
    if (!player2 || playerVideoId(player2) !== id) return {};
    const facts = {};
    try {
      const data = player2.getVideoData?.();
      facts.title = nonEmpty(pickString(data, "title"));
      facts.artist = nonEmpty(pickString(data, "author"));
      facts.live = pick(data, "isLive") === true;
      const duration = player2.getDuration?.();
      if (typeof duration === "number" && Number.isFinite(duration) && duration > 0) {
        facts.seconds = duration;
      }
    } catch {
    }
    const video = readVideo(player2);
    if (video?.id === id) {
      facts.title ??= nonEmpty(video.title);
      facts.artist ??= nonEmpty(video.author);
      facts.live ||= video.live;
      if (!facts.seconds && video.lengthSeconds > 0) facts.seconds = video.lengthSeconds;
    }
    return facts;
  }
  function fromPage(id) {
    const details = pick(window.ytInitialPlayerResponse, "videoDetails");
    if (pickString(details, "videoId") === id) {
      return {
        title: nonEmpty(pickString(details, "title")),
        artist: nonEmpty(pickString(details, "author")),
        seconds: Number(pick(details, "lengthSeconds")) || void 0,
        live: pick(details, "isLive") === true
      };
    }
    if (watchVideoId(loadedHref) !== id) return {};
    const meta = (selector) => nonEmpty(document.querySelector(selector)?.getAttribute("content"));
    return {
      title: meta('meta[name="title"]') ?? meta('meta[property="og:title"]'),
      artist: meta('[itemprop="author"] [itemprop="name"]')
    };
  }
  function fromMediaSession(title) {
    const metadata = navigator.mediaSession?.metadata;
    if (!metadata || !title || metadata.title !== title) return {};
    const artwork = [...metadata.artwork].reverse().find((picture) => picture.src.startsWith("https://"));
    return {
      artist: nonEmpty(metadata.artist),
      album: nonEmpty(metadata.album),
      artworkUrl: artwork?.src
    };
  }
  function videoOffer(id) {
    const player2 = getPlayer();
    const playerFacts = fromPlayer(player2, id);
    const pageFacts = fromPage(id);
    const title = playerFacts.title ?? pageFacts.title;
    const sessionFacts = isMusic ? fromMediaSession(title) : {};
    const author = sessionFacts.artist ?? playerFacts.artist ?? pageFacts.artist;
    const live = playerFacts.live || pageFacts.live;
    const seconds = live ? void 0 : playerFacts.seconds ?? pageFacts.seconds;
    const offer = {
      kind: isMusic ? "song" : "video",
      url: `${SITE}/watch?v=${id}`,
      artworkUrl: sessionFacts.artworkUrl ?? `https://i.ytimg.com/vi/${id}/hqdefault.jpg`
    };
    if (title) offer.title = title;
    const artist = isMusic ? artistName(author) : author;
    if (artist) offer.artist = artist;
    if (sessionFacts.album) offer.album = sessionFacts.album;
    if (seconds && seconds > 0) offer.durationMs = Math.round(seconds) * 1e3;
    return offer;
  }
  function playlistTitleIn(response, listId) {
    const candidates = [
      pick(response, "contents", "twoColumnWatchNextResults", "playlist", "playlist"),
      pick(response, "contents", "singleColumnWatchNextResults", "playlist", "playlist"),
      pick(
        response,
        "contents",
        "singleColumnMusicWatchNextResultsRenderer",
        "tabbedRenderer",
        "watchNextTabbedResultsRenderer",
        "tabs",
        0,
        "tabRenderer",
        "content",
        "musicQueueRenderer",
        "content",
        "playlistPanelRenderer"
      )
    ];
    for (const playlist of candidates) {
      if (!isObject(playlist) || playlist.playlistId !== listId) continue;
      const title = nonEmpty(textOf(playlist.title));
      if (title) return title;
    }
    return void 0;
  }
  function playlistPageTitle(listId) {
    const data = window.ytInitialData;
    const canonical = pickString(data, "microformat", "microformatDataRenderer", "urlCanonical");
    if (!canonical?.includes(`list=${listId}`)) return void 0;
    return nonEmpty(pickString(data, "metadata", "playlistMetadataRenderer", "title")) ?? nonEmpty(pickString(data, "microformat", "microformatDataRenderer", "title"));
  }
  var playlistTitles = /* @__PURE__ */ new Map();
  function playlistOffer(listId) {
    let title = playlistTitles.get(listId);
    if (!title) {
      let response;
      try {
        response = getPlayer()?.getWatchNextResponse?.();
      } catch {
        response = void 0;
      }
      title = playlistTitleIn(response, listId) ?? playlistTitleIn(window.ytInitialData, listId) ?? playlistPageTitle(listId);
      if (title) playlistTitles.set(listId, title);
    }
    const offer = { kind: "playlist", url: `${SITE}/playlist?list=${listId}` };
    if (title) offer.title = title;
    return offer;
  }
  function currentOffers() {
    const offers = [];
    const url = new URL(location.href);
    const videoId2 = watchVideoId();
    if (videoId2) offers.push(videoOffer(videoId2));
    const listId = url.searchParams.get("list") ?? "";
    const listPage = url.pathname === "/watch" || url.pathname === "/playlist";
    if (listPage && LIST_ID.test(listId) && !isPersonalList(listId)) {
      offers.push(playlistOffer(listId));
    }
    return offers;
  }
  var offered = "";
  function refresh() {
    let offers;
    try {
      offers = currentOffers();
    } catch (error) {
      console.error("[YouTube enhancements] offers:", error);
      return;
    }
    const key = JSON.stringify(offers);
    if (key === offered) return;
    if (offered) raygunPage.withdraw();
    offered = key;
    for (const offer of offers) raygunPage.offer(offer);
  }
  function installOffers() {
    refresh();
    onNavigate(refresh);
    onVideoChange(() => refresh());
    onPlayerState(() => refresh());
    setInterval(refresh, 1e3);
  }

  // src/lib/playlist-nav.ts
  var getVideoId2 = (entry) => pickString(entry, "playlistPanelVideoRenderer", "videoId");
  var getSelectedId = (playlist) => getVideoId2(
    pickArray(playlist, "contents").find(
      (entry) => Boolean(pick(entry, "playlistPanelVideoRenderer", "selected"))
    )
  );
  var isPlaylistCurrent = (playlist, playingVideoId) => Boolean(playingVideoId) && getSelectedId(playlist) == playingVideoId;
  function getPlaylists(response) {
    return ["singleColumnWatchNextResults", "twoColumnWatchNextResults"].map((layout) => pick(response, "contents", layout, "playlist", "playlist")).filter((playlist) => Array.isArray(pick(playlist, "contents")));
  }
  function getPlayingPlaylist(response, playingVideoId) {
    return getPlaylists(response).find((playlist) => isPlaylistCurrent(playlist, playingVideoId));
  }
  function getHeldPlaylist(responses, playingVideoId, listId) {
    if (!playingVideoId || !listId) return void 0;
    const playlists = responses.flatMap(getPlaylists).filter((playlist) => pick(playlist, "playlistId") == listId);
    return playlists.find((playlist) => isPlaylistCurrent(playlist, playingVideoId)) || playlists.find(
      (playlist) => pickArray(playlist, "contents").some((entry) => getVideoId2(entry) == playingVideoId)
    );
  }
  function getEndpointUrl(endpoint, playlistId) {
    const url = pickString(endpoint, "commandMetadata", "webCommandMetadata", "url");
    if (url?.includes("v=")) return url;
    const videoId2 = pickString(endpoint, "watchEndpoint", "videoId");
    if (!videoId2) return void 0;
    const list = pickString(endpoint, "watchEndpoint", "playlistId") || playlistId;
    return `/watch?v=${videoId2}${list ? `&list=${list}` : ""}`;
  }
  function getPlaylistNeighborUrl(data, direction, playingVideoId) {
    if (!isObject(data)) return void 0;
    const contents = pickArray(data, "contents");
    const playlistId = pickString(data, "playlistId");
    const selectedId = getSelectedId(data);
    const playingId = playingVideoId || selectedId;
    const current = Boolean(selectedId) && selectedId == playingId;
    const playingIndex = contents.findIndex(
      (entry) => current ? Boolean(pick(entry, "playlistPanelVideoRenderer", "selected")) : getVideoId2(entry) == playingId
    );
    if (!playingId || playingIndex < 0) return void 0;
    const button = direction == "next" ? data.nextButtonVideo : data.previousButtonVideo;
    const command = pick(button, "buttonRenderer", "command");
    if (current && pickString(command, "watchEndpoint", "videoId") != selectedId) {
      const url = getEndpointUrl(command, playlistId);
      if (url) return url;
    }
    const neighbor = pick(
      contents[playingIndex + (direction == "next" ? 1 : -1)],
      "playlistPanelVideoRenderer"
    );
    const neighborId = pickString(neighbor, "videoId");
    if (!neighborId) return void 0;
    return getEndpointUrl(pick(neighbor, "navigationEndpoint"), playlistId) || getEndpointUrl({ watchEndpoint: { videoId: neighborId } }, playlistId);
  }

  // src/playlist-nav.ts
  function navigateWatch(url) {
    try {
      const target = new URL(url, location.href);
      target.host = location.host;
      const anchor = document.createElement("a");
      anchor.href = target.href;
      anchor.style.display = "none";
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
    } catch {
      location.href = url;
    }
  }
  function skipInPlaylist(direction) {
    const player2 = getPlayer();
    if (!player2) return false;
    const videoId2 = playerVideoId(player2) || void 0;
    let response;
    try {
      response = player2.getWatchNextResponse?.();
    } catch {
      response = void 0;
    }
    const playlist = getPlayingPlaylist(response, videoId2) || getHeldPlaylist(
      [response, window.ytInitialData],
      videoId2,
      new URLSearchParams(location.search).get("list")
    );
    if (!playlist) return false;
    const url = getPlaylistNeighborUrl(playlist, direction, videoId2);
    if (!url) return true;
    if (isPlaylistCurrent(playlist, videoId2)) {
      navigateWatch(url);
    } else {
      location.assign(url);
    }
    return true;
  }
  function installPlaylistControls() {
    hookMediaAction("nexttrack", () => skipInPlaylist("next"));
    hookMediaAction("previoustrack", () => skipInPlaylist("previous"));
  }

  // src/sponsorblock.ts
  var API = "https://sponsor.ajay.app/api/skipSegments/";
  var categoryColors = {
    sponsor: "#00d400",
    selfpromo: "#ffff00",
    interaction: "#cc00ff",
    intro: "#00ffff",
    outro: "#0202ed",
    preview: "#008fd6",
    music_offtopic: "#ff9900",
    poi_highlight: "#ff1684",
    filler: "#7300ff"
  };
  var overlayClass = "_nou_sb_segments";
  var segmentClass = "_nou_sb_segment";
  var progressBarSelectors = [
    ".ytChapteredProgressBarHost",
    ".ytp-progress-list",
    ".progress-bar-line",
    ".progress-bar-background"
  ];
  var chapterSelectors = [
    ".ytChapteredProgressBarChapteredPlayerBarChapter",
    ".ytp-chapter-hover-container"
  ];
  function getProgressBars() {
    const bars = [];
    for (const selector of progressBarSelectors) {
      for (const element of document.querySelectorAll(selector)) {
        if (element.offsetWidth > 0) bars.push(element);
      }
    }
    return bars;
  }
  function getChapters(bar) {
    let elements = [];
    for (const selector of chapterSelectors) {
      elements = Array.from(bar.querySelectorAll(selector));
      if (elements.length) break;
    }
    if (!elements.length) return [{ start: 0, end: 1, left: 0, width: bar.offsetWidth }];
    const totalWidth = elements.reduce((sum, element) => sum + element.offsetWidth, 0);
    const chapters = [];
    let start2 = 0;
    for (const element of elements) {
      const percent = parseFloat(element.style.width);
      const span = element.style.width.endsWith("%") && Number.isFinite(percent) ? percent / 100 : element.offsetWidth / (totalWidth || 1);
      chapters.push({
        start: start2,
        end: start2 + span,
        left: element.offsetLeft,
        width: element.offsetWidth
      });
      start2 += span;
    }
    const last = chapters[chapters.length - 1];
    if (last) last.end = 1;
    return chapters;
  }
  function clearSkipSegments() {
    for (const element of document.querySelectorAll(`.${overlayClass}`)) {
      element.parentElement?.removeAttribute("data-nou-sb");
      element.remove();
    }
  }
  function renderSkipSegments(videoId2, segments2, duration) {
    if (!videoId2 || !duration || !Number.isFinite(duration)) return;
    const segmentKey = segments2.map(
      ({ actionType, category, segment }) => `${actionType}:${category}:${segment[0]}:${segment[1]}`
    ).join(",");
    for (const bar of getProgressBars()) {
      const overlay = bar.querySelector(`.${overlayClass}`);
      const chapters = getChapters(bar);
      const chapterKey = chapters.map(({ start: start2, end, left, width }) => `${start2}:${end}:${left}:${width}`).join(",");
      const key = `${videoId2}:${duration}:${segmentKey}:${bar.offsetWidth}:${chapterKey}`;
      if (overlay && bar.dataset.nouSb === key) continue;
      overlay?.remove();
      if (getComputedStyle(bar).position == "static") bar.style.position = "relative";
      bar.append(buildOverlay(segments2, duration, chapters));
      bar.dataset.nouSb = key;
    }
  }
  function buildOverlay(segments2, duration, chapters) {
    const overlay = document.createElement("div");
    overlay.className = overlayClass;
    for (const segment of segments2) {
      if (segment.actionType == "full") continue;
      const background = categoryColors[segment.category] || categoryColors.sponsor || "";
      const start2 = Math.max(0, Math.min(1, segment.segment[0] / duration));
      const end = Math.max(start2, Math.min(1, segment.segment[1] / duration));
      for (const chapter of chapters) {
        const from = Math.max(start2, chapter.start);
        const to = Math.min(end, chapter.end);
        if (start2 == end ? to < from : to <= from) continue;
        const span = chapter.end - chapter.start;
        if (span <= 0) continue;
        const width = (to - from) / span * chapter.width;
        if (start2 != end && width < 0.5) continue;
        const element = document.createElement("div");
        element.className = segmentClass;
        element.style.left = `${chapter.left + (from - chapter.start) / span * chapter.width}px`;
        element.style.width = `${Math.max(start2 == end ? 2 : 1, width)}px`;
        element.style.background = background;
        overlay.append(element);
      }
    }
    return overlay;
  }
  var toSegment = (value) => {
    const range = pickArray(value, "segment");
    const [from, to] = range;
    if (typeof from !== "number" || typeof to !== "number") return void 0;
    return {
      category: pickString(value, "category") ?? "sponsor",
      actionType: pickString(value, "actionType") ?? "skip",
      segment: [from, to]
    };
  };
  async function getSkipSegments(videoId2) {
    const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(videoId2));
    const prefix = Array.from(
      new Uint8Array(digest).slice(0, 2),
      (byte) => byte.toString(16).padStart(2, "0")
    ).join("");
    const query = `categories=${encodeURIComponent('["sponsor"]')}&actionTypes=${encodeURIComponent('["skip"]')}`;
    const response = await pageFetch(`${API}${prefix}?${query}`);
    if (response.status !== 200) return [];
    const videos = await response.json();
    const video = (Array.isArray(videos) ? videos : []).find(
      (entry) => isObject(entry) && entry.videoID === videoId2
    );
    return pickArray(video, "segments").map(toSegment).filter((segment) => segment !== void 0);
  }
  var segmentsFor = "";
  var segments = [];
  var requested = "";
  function load(player2) {
    const videoId2 = playerVideoId(player2);
    if (!videoId2 || videoId2 === requested || !playsMainVideo(player2)) return;
    requested = videoId2;
    segmentsFor = "";
    segments = [];
    clearSkipSegments();
    getSkipSegments(videoId2).then((found) => {
      if (requested !== videoId2) return;
      segmentsFor = videoId2;
      segments = found;
      redraw();
    }).catch((error) => {
      console.warn("[YouTube enhancements] SponsorBlock:", error);
    });
  }
  var durationOf = (player2) => {
    try {
      return player2.getDuration?.() ?? 0;
    } catch {
      return 0;
    }
  };
  function redraw() {
    const player2 = getPlayer();
    if (!player2 || !segments.length || playerVideoId(player2) !== segmentsFor) return;
    renderSkipSegments(segmentsFor, segments, durationOf(player2));
  }
  function installSponsorBlock() {
    onVideoChange((_video, player2) => {
      clearSkipSegments();
      if (player2) load(player2);
    });
    let lastDrawn = 0;
    onTimeUpdate((player2, media2) => {
      load(player2);
      if (playerVideoId(player2) !== segmentsFor || !segments.length) return;
      const now = Date.now();
      if (now - lastDrawn > 1e3) {
        lastDrawn = now;
        renderSkipSegments(segmentsFor, segments, durationOf(player2));
      }
      const time = media2.currentTime;
      for (const { actionType, segment } of segments) {
        if (actionType !== "skip") continue;
        const [start2, end] = segment;
        if (time > start2 && time < end) {
          player2.seekTo?.(end, true);
          return;
        }
      }
    });
    window.addEventListener("resize", redraw);
    document.addEventListener("fullscreenchange", () => {
      redraw();
      setTimeout(redraw, 400);
    });
  }

  // src/watch-nav.ts
  function isWatch(address) {
    try {
      return new URL(address, location.href).pathname.startsWith("/watch");
    } catch {
      return false;
    }
  }
  function resolveHistoryUrl(url) {
    if (typeof url === "string" && url || url instanceof URL) {
      try {
        return new URL(url, location.href).href;
      } catch {
        return location.href;
      }
    }
    return location.href;
  }
  function installWatchNavigation() {
    const pushState = history.pushState;
    history.pushState = function(data, unused, url) {
      if (isWatch(location.href) && isWatch(resolveHistoryUrl(url))) {
        return history.replaceState.call(this, data, unused, url);
      }
      return pushState.call(this, data, unused, url);
    };
  }

  // src/youtube.ts
  function install(part, run) {
    try {
      run();
    } catch (error) {
      console.error(`[YouTube enhancements] ${part}:`, error);
    }
  }
  function start() {
    const blockAds = flag("blockAds", true);
    const backgroundPlay = flag("backgroundPlay", true);
    const hideShorts = flag("hideShorts", true) && !isMusic;
    const showOriginalTitles = flag("showOriginalTitles", false);
    const blocklist = blocklistFromSettings(
      textSetting("hiddenChannels"),
      textSetting("hiddenWords")
    );
    const hidesChannels = !isMusic && !isBlocklistEmpty(blocklist);
    const guardBackground = backgroundPlay && isAndroid;
    if (guardBackground || isMobileSite) install("media controls", wrapMediaSession);
    if (guardBackground) install("background play", guardMediaSessionHandlers);
    if (isMobileSite) install("playlist controls", installPlaylistControls);
    if (flag("preferH264", false)) install("H.264", installH264ify);
    const thumbnails = thumbnailTarget(textSetting("thumbnails"));
    if (thumbnails) install("thumbnails", () => installClickbaitThumbnails(thumbnails));
    if (blockAds || hideShorts || showOriginalTitles || hidesChannels) {
      install(
        "filters",
        () => installIntercept(
          { blockAds, hideShorts, showOriginalTitles },
          hidesChannels ? blocklist : void 0
        )
      );
    }
    if (flag("backSkipsWatched", false)) install("history", installWatchNavigation);
    install("styles", injectCss);
    if (backgroundPlay) install("session", keepSessionActive);
    if (guardBackground) install("background play", installBackgroundGuard);
    if (flag("sponsorBlock", true)) install("SponsorBlock", installSponsorBlock);
    if (flag("originalAudio", true)) install("original audio", installOriginalAudio);
    if (flag("dislikes", false) && !isMusic) install("dislikes", installDislikeCount);
    if (flag("liveChat", true) && isMobileSite) install("live chat", installLiveChat);
    if (!isMusic) {
      install("autoplay", () => installAutoplayPolicy(flag("disableAutoplay", false)));
    }
    if (hidesChannels) install("hidden channels", () => installBlocklistFilter(blocklist));
    if (isMobileSite) install("channel names", installEncodedAuthorNameFix);
    if (isMusic && blockAds) install("promotions", installPromoDismissal);
    install("offers", installOffers);
    install("player", watchPlayer);
  }
  if (!window.__nouRaygun) {
    window.__nouRaygun = true;
    start();
  }
})();
