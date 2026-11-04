// dev.raygun.wikimedia-commons 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/commons.ts
  var COMMONS = "https://commons.wikimedia.org";
  var API = `${COMMONS}/w/api.php`;
  var AGENT = "Raygun/1.0 (https://github.com/uvxszi/raygun)";
  var MEDIA_HEADERS = { "User-Agent": AGENT };
  var API_HEADERS = { "User-Agent": AGENT, "Api-User-Agent": AGENT };
  var FIELDS = [
    "ObjectName",
    "ImageDescription",
    "Artist",
    "LicenseShortName",
    "License",
    "Categories",
    "DateTimeOriginal",
    "DateTime"
  ].join("|");
  var api = async (params) => {
    const query = Object.entries({ format: "json", formatversion: 2, ...params }).map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`).join("&");
    const response = await raygun.http.fetch(`${API}?${query}`, { headers: API_HEADERS });
    if (!response.ok) throw new Error(`Wikimedia Commons answered HTTP ${response.status}`);
    const answer = await response.json();
    if (answer.error) {
      throw new Error(`Wikimedia Commons: ${answer.error.info ?? "the request failed"}`);
    }
    return answer;
  };
  var fileProps = (withTracks) => ({
    prop: "videoinfo",
    viprop: `url|size|mime|mediatype|extmetadata|derivatives${withTracks ? "|timedtext" : ""}`,
    viurlwidth: 480,
    viextmetadatafilter: FIELDS,
    viextmetadatamultilang: 0
  });
  var searchFiles = async (search2, { limit, offset = 0, sort = "relevance" }) => {
    const answer = await api({
      action: "query",
      generator: "search",
      gsrnamespace: 6,
      gsrsearch: search2,
      gsrlimit: limit,
      gsroffset: offset,
      gsrsort: sort,
      ...fileProps(false)
    });
    const files = [...answer.query?.pages ?? []].sort((a, b) => (a.index ?? 0) - (b.index ?? 0));
    return { files, more: Boolean(answer.continue?.gsroffset) };
  };
  var fileByName = async (name) => {
    const answer = await api({
      action: "query",
      titles: `File:${name}`,
      ...fileProps(true)
    });
    const file = answer.query?.pages?.[0];
    if (!file || file.missing || !file.videoinfo?.[0]) {
      throw new Error("This file is no longer on Wikimedia Commons");
    }
    return file;
  };
  var decode = (text) => text.replace(/&nbsp;|&#160;/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#0?39;|&apos;/g, "'").replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(parseInt(code, 16))
  ).replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&amp;/g, "&");
  var plainText = (html) => decode(
    html.replace(/<br\s*\/?>/gi, "\n").replace(/<\/p>/gi, "\n\n").replace(/<[^>]+>/g, "")
  ).replace(/[^\S\n]+/g, " ").replace(/\n\s*\n\s*/g, "\n\n").trim();
  var field = (info, key) => {
    const value = info.extmetadata?.[key]?.value;
    return value === void 0 ? "" : plainText(String(value));
  };
  var openlyLicensed = (info) => /^(cc|pd)/i.test(field(info, "License")) || /^(CC[ -]|CC0|Public domain|PD\b)/i.test(field(info, "LicenseShortName"));
  var ADULT = /\b(nude|nudes|nudity|nudist|nudism|naked|sex|sexual|sexuality|porn\w*|erotic\w*|genital\w*|penis|vagina|vulva|breasts?|nipples?|masturbat\w*|intercourse|fetish\w*|bdsm|striptease|topless)\b/i;
  var adult = (info) => ADULT.test(field(info, "Categories"));
  var nameOf = (file) => file.title.replace(/^File:/, "");
  var titleOf = (file) => {
    const info = file.videoinfo?.[0];
    const named = info ? field(info, "ObjectName") : "";
    return named || nameOf(file).replace(/\.[^.]+$/, "").replace(/_/g, " ");
  };
  var creatorOf = (info) => {
    const line = (field(info, "Artist").split("\n")[0] ?? "").split(/;|\.\s/)[0].replace(/^(credit|author|artist|by|produced by|directed by)\s*:?\s*/i, "").trim();
    if (!line || /^(unknown|see above|see below|anonymous)/i.test(line) || line.length > 80) {
      return null;
    }
    return line;
  };
  var clean = (url) => url ? url.replace(/[?&]utm_[^#]*$/, "") : void 0;

  // src/audio.ts
  var AUDIO = "filetype:audio -filemime:audio/midi filesize:>300";
  var MIN_SECONDS = 30;
  var PLAYABLE = /^(audio\/(mpeg|ogg|flac|x-flac|wav|x-wav|webm|opus)|application\/ogg)/;
  var words = (text) => text.replace(/[\\"~*?:()[\]{}^|/-]/g, " ").replace(/\s+/g, " ").trim();
  var bareTitle = (title) => title.replace(/\([^)]*\)|\[[^\]]*\]/g, " ").trim() || title;
  var mainArtist = (artist) => artist.split(/\s*(?:&|,|;|\/|\bfeat\.?|\bft\.?|\band\b|\bwith\b)\s*/i)[0]?.trim() || artist;
  var wordsOf = (text) => words(text).toLowerCase().split(" ").filter(Boolean);
  var streamOf = (info) => {
    const mp3 = info.derivatives?.find((copy) => copy.transcodekey === "mp3" && copy.src);
    if (mp3) {
      return {
        source: { kind: "url", url: mp3.src, headers: MEDIA_HEADERS },
        quality: "MP3",
        bitrateKbps: mp3.bandwidth ? Math.round(mp3.bandwidth / 1e3) : null
      };
    }
    const url = clean(info.url);
    if (!url || !PLAYABLE.test(info.mime ?? "")) return null;
    const format = /flac/.test(info.mime ?? "") ? "FLAC" : (info.mime ?? "").split("/")[1]?.toUpperCase();
    return {
      source: { kind: "url", url, headers: MEDIA_HEADERS },
      quality: format ?? "Audio",
      bitrateKbps: format === "FLAC" ? 1411 : null
    };
  };
  var songs = (files) => files.flatMap((file) => {
    const info = file.videoinfo?.[0];
    if (!info || info.mediatype !== "AUDIO" || /midi/.test(info.mime ?? "")) return [];
    if (!openlyLicensed(info) || adult(info) && !raygun.content.showNsfw()) return [];
    if ((info.duration ?? 0) < MIN_SECONDS) return [];
    const stream = streamOf(info);
    if (!stream) return [];
    return [
      {
        stream,
        track: {
          id: nameOf(file),
          title: titleOf(file),
          artist: creatorOf(info),
          album: null,
          durationMs: Math.round((info.duration ?? 0) * 1e3),
          artworkUrl: null,
          source: stream.source
        }
      }
    ];
  });
  var search = defineProvider({
    kind: "music.metadata",
    id: "audio",
    name: "Wikimedia Commons",
    async search({ query, limit }) {
      const asked = words(query);
      if (asked === "") return { tracks: [] };
      const { files } = await searchFiles(`${AUDIO} ${asked}`, { limit: Math.min(50, limit * 2) });
      return {
        tracks: songs(files).slice(0, limit).map((song) => song.track)
      };
    }
  });
  var streams = defineProvider({
    kind: "music.stream",
    id: "stream",
    name: "Wikimedia Commons",
    async candidates({ title, artist, limit }) {
      const wanted = wordsOf(bareTitle(title));
      if (wanted.length === 0) return { candidates: [] };
      const named = words(bareTitle(title));
      const by = artist ? words(mainArtist(artist)) : "";
      const searches = by ? [`${AUDIO} ${named} ${by}`, `${AUDIO} ${named}`] : [`${AUDIO} ${named}`];
      const answers = await Promise.allSettled(
        searches.map((search2) => searchFiles(search2, { limit: 20 }))
      );
      const failures = answers.flatMap(
        (answer) => answer.status === "rejected" ? [answer.reason] : []
      );
      if (failures.length === answers.length) throw failures[0];
      const files = [
        ...new Map(
          answers.flatMap((answer) => answer.status === "fulfilled" ? answer.value.files : []).map((file) => [file.title, file])
        ).values()
      ];
      const matching = songs(files).filter(({ track }) => {
        const offered = wordsOf(track.title);
        return wanted.every((word) => offered.includes(word));
      });
      return {
        candidates: matching.slice(0, limit).map(({ track, stream }) => ({
          id: String(track.id),
          title: track.title,
          artist: track.artist ?? null,
          durationMs: track.durationMs ?? null,
          quality: stream.quality,
          bitrateKbps: stream.bitrateKbps,
          source: stream.source
        }))
      };
    }
  });

  // src/videos.ts
  var PAGE_SIZE = 24;
  var FEATURED = 'incategory:"Media_of_the_day"';
  var COLLECTIONS = [
    { value: "all", label: "All of Commons" },
    { value: "featured", label: "Media of the day" }
  ];
  var SORTS = [
    { value: "relevance", label: "Best match" },
    { value: "incoming_links_desc", label: "Most used" },
    { value: "create_timestamp_desc", label: "Newest" }
  ];
  var words2 = (text) => text.replace(/[\\"~*?:()[\]{}^|/-]/g, " ").replace(/\s+/g, " ").trim();
  var isTopic = (name) => !/(videos?$|^videos?\b|^audio\b|^\d{4}(-\d\d){0,2}$|^media\b|^files\b|^uploaded|self-published|^cc-|^pd-|resolution|^webm|^ogg|^vp\d|^opus|^av1|captioning|^license|^quality|^featured|^valued|^pages with|^commons|wikipedia|youtube|flickr|vimeo)/i.test(
    name
  );
  var listed = (file) => {
    const info = file.videoinfo?.[0];
    return info !== void 0 && info.mediatype === "VIDEO" && openlyLicensed(info) && (!adult(info) || raygun.content.showNsfw());
  };
  var listPage = async (search2, sort, number) => {
    const offset = (Math.max(1, number) - 1) * PAGE_SIZE;
    const { files, more } = await searchFiles(search2, { limit: PAGE_SIZE, offset, sort });
    return {
      items: files.filter(listed).map((file) => ({
        id: nameOf(file),
        title: titleOf(file),
        coverUrl: clean(file.videoinfo?.[0]?.thumburl) ?? null,
        nsfw: file.videoinfo?.[0] ? adult(file.videoinfo[0]) : false
      })),
      hasNextPage: more
    };
  };
  var pick = (filters2, key, options) => {
    const value = filters2?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var nominalHeight = (key) => Number.parseInt(key ?? "", 10) || void 0;
  var videosOf = (info) => {
    const subtitles = (info.timedtext ?? []).filter((track) => track.src && track.kind !== "metadata").map((track) => ({ url: track.src, lang: track.srclang, label: track.label }));
    const copies = (info.derivatives ?? []).filter((copy) => /^video\/webm/.test(copy.type ?? "") && /vp9|vp8|av01/i.test(copy.type ?? "")).map((copy) => {
      const height = nominalHeight(copy.transcodekey) ?? copy.height;
      const codec = /vp9/i.test(copy.type ?? "") ? "VP9" : /av01/i.test(copy.type ?? "") ? "AV1" : "VP8";
      return {
        url: copy.src,
        label: height ? `${height}p · ${codec}` : codec,
        quality: height,
        headers: MEDIA_HEADERS,
        subtitles
      };
    });
    const original = clean(info.url);
    if (original && info.mime === "video/webm") {
      copies.push({
        url: original,
        label: info.height ? `Original · ${info.width}×${info.height}` : "Original",
        quality: info.height || void 0,
        headers: MEDIA_HEADERS,
        subtitles
      });
    }
    return copies.sort((a, b) => (b.quality ?? 0) - (a.quality ?? 0));
  };
  var filters = [
    { kind: "select", key: "collection", label: "Collection", options: COLLECTIONS },
    { kind: "select", key: "sort", label: "Sort", options: SORTS }
  ];
  var server = async (name) => {
    const info = (await fileByName(name)).videoinfo[0];
    const videos2 = videosOf(info);
    if (videos2.length === 0)
      throw new Error("Commons has no copy of this video that Raygun can play");
    return { id: name, name: "Wikimedia Commons", videos: videos2 };
  };
  var videos = defineProvider({
    kind: "video.source",
    id: "videos",
    name: "Wikimedia Commons",
    popular: ({ page: number }) => listPage(`filetype:video ${FEATURED}`, "incoming_links_desc", number),
    latest: ({ page: number }) => listPage(`filetype:video ${FEATURED}`, "create_timestamp_desc", number),
    search: ({ query, page: number, filters: values }) => {
      const asked = words2(query);
      const featured = pick(values, "collection", COLLECTIONS) === "featured" || asked === "";
      const sort = pick(values, "sort", SORTS) ?? (asked ? "relevance" : "incoming_links_desc");
      const search2 = ["filetype:video", featured && FEATURED, asked].filter(Boolean).join(" ");
      return listPage(search2, sort, number);
    },
    filters: async () => filters,
    async details({ id }) {
      const file = await fileByName(id);
      const info = file.videoinfo[0];
      const about = field(info, "ImageDescription");
      const licence = field(info, "LicenseShortName");
      const creator = creatorOf(info);
      const year = /\b(1[89]\d\d|20\d\d)\b/.exec(
        `${field(info, "DateTimeOriginal")} ${field(info, "DateTime")}`
      )?.[1];
      const topics = field(info, "Categories").split("|").map((name) => name.trim()).filter((name) => name && isTopic(name));
      return {
        id,
        title: titleOf(file),
        coverUrl: clean(info.thumburl) ?? null,
        description: [
          about.slice(0, 2e3),
          [licence && `Licence: ${licence}`, creator && `By ${creator}`].filter(Boolean).join(" · ")
        ].filter(Boolean).join("\n\n") || null,
        genres: topics.slice(0, 6),
        directors: creator ? [creator] : [],
        type: "other",
        year: year ? Number(year) : null,
        status: "completed",
        url: `${COMMONS}/wiki/File:${encodeURIComponent(id.replace(/ /g, "_"))}`
      };
    },
    episodes: async ({ id }) => ({ episodes: [{ id, name: "Video" }] }),
    servers: async ({ episodeId }) => ({ servers: [await server(episodeId)] }),
    videos: async ({ serverId }) => ({ videos: (await server(serverId)).videos })
  });

  // src/index.ts
  registerProvider(search);
  registerProvider(streams);
  registerProvider(videos);
})();
