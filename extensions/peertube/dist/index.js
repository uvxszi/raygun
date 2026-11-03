// dev.raygun.peertube 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var fetchJson = async (url, init) => {
    const response = await raygun.http.fetch(url, init);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} from ${url}`);
    }
    return response.json();
  };

  // src/index.ts
  var SEARCH = "https://sepiasearch.org/api/v1/search/videos";
  var COUNT = 24;
  var idOf = (found) => {
    const host = /^https?:\/\/([^/?#]+)/i.exec(found.url)?.[1] ?? "";
    return `${host}/${found.uuid}`;
  };
  var splitId = (id) => {
    const slash = id.indexOf("/");
    if (slash < 1) throw new Error("Not a PeerTube video id");
    const host = id.slice(0, slash);
    if (!/^[a-z0-9.-]+(:\d+)?$/i.test(host)) throw new Error("Not a PeerTube instance");
    return { host, uuid: id.slice(slash + 1) };
  };
  var searchPage = async (query, sort, page) => {
    const params = [
      `search=${encodeURIComponent(query)}`,
      `start=${(Math.max(1, page) - 1) * COUNT}`,
      `count=${COUNT}`,
      `sort=${sort}`,
      `nsfw=${raygun.content.showNsfw() ? "both" : "false"}`
    ].join("&");
    const result = await fetchJson(`${SEARCH}?${params}`);
    const items = (result.data ?? []).filter((found) => raygun.content.showNsfw() || !found.nsfw).map((found) => ({
      id: idOf(found),
      title: found.isLive ? `● ${found.name}` : found.name,
      coverUrl: found.thumbnailUrl ?? null,
      nsfw: found.nsfw ?? false
    }));
    return { items, hasNextPage: page * COUNT < (result.total ?? 0) };
  };
  var details = async (id) => {
    const { host, uuid } = splitId(id);
    const video = await fetchJson(
      `https://${host}/api/v1/videos/${encodeURIComponent(uuid)}`
    );
    return { host, video };
  };
  var linksOf = (video) => {
    const links = [];
    for (const playlist of video.streamingPlaylists ?? []) {
      if (playlist.playlistUrl) {
        links.push({ url: playlist.playlistUrl, label: "HLS (every quality)", format: "hls" });
      }
    }
    const files = [
      ...video.files ?? [],
      ...(video.streamingPlaylists ?? []).flatMap((p) => p.files ?? [])
    ].filter((file) => file.fileUrl && /^https?:\/\//.test(file.fileUrl)).sort((a, b) => b.resolution.id - a.resolution.id);
    for (const file of files) {
      links.push({
        url: file.fileUrl,
        label: file.resolution.label ?? `${file.resolution.id}p`,
        quality: file.resolution.id || void 0
      });
    }
    return links;
  };
  var peertube = defineProvider({
    kind: "video.source",
    id: "videos",
    name: "PeerTube",
    searchPill: "PeerTube",
    popular: ({ page }) => searchPage("", "-views", page),
    latest: ({ page }) => searchPage("", "-publishedAt", page),
    search: ({ query, page }) => searchPage(query.trim(), query.trim() ? "-match" : "-views", page),
    async details({ id }) {
      const { host, video } = await details(id);
      const cover = video.previewPath ?? video.thumbnailPath;
      const by = video.channel?.displayName ?? video.account?.displayName;
      const year = Number.parseInt((video.publishedAt ?? "").slice(0, 4), 10);
      return {
        id,
        title: video.name,
        coverUrl: cover ? `https://${host}${cover}` : null,
        description: [by && `By ${by} on ${host}`, video.description].filter(Boolean).join("\n\n"),
        genres: [video.category?.label, ...video.tags ?? []].filter(
          (tag) => Boolean(tag)
        ),
        type: "movie",
        year: Number.isFinite(year) ? year : null,
        status: video.isLive ? "ongoing" : "completed",
        url: `https://${host}/w/${splitId(id).uuid}`
      };
    },
    episodes: async ({ id }) => ({ episodes: [{ id, name: "Video" }] }),
    servers: async ({ episodeId }) => {
      const { host, video } = await details(episodeId);
      return { servers: [{ id: episodeId, name: host, videos: linksOf(video) }] };
    },
    videos: async ({ serverId }) => ({ videos: linksOf((await details(serverId)).video) })
  });
  registerProvider(peertube);
})();
