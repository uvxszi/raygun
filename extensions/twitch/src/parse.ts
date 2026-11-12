// What Twitch's API (Helix, https://dev.twitch.tv/docs/api/reference/) answers, read into
// Raygun's catalogue shapes. Pure: no requests, so the tests read fixtures with it.
import type { MangaSummary, Video, VideoDetails, VideoEpisode } from '@raygun/sdk';

/** `GET /streams`: a stream on the air now. */
export type HelixStream = {
  id: string;
  user_id: string;
  user_login: string;
  user_name: string;
  game_id?: string;
  game_name?: string;
  type?: string;
  title?: string;
  tags?: string[] | null;
  viewer_count?: number;
  started_at?: string;
  language?: string;
  /** `…/live_user_<login>-{width}x{height}.jpg`. */
  thumbnail_url?: string;
};

/** `GET /search/channels`: a channel, live or not. */
export type HelixChannelResult = {
  id: string;
  broadcaster_login: string;
  display_name: string;
  broadcaster_language?: string;
  game_id?: string;
  game_name?: string;
  is_live?: boolean;
  tags?: string[] | null;
  /** The channel's picture. */
  thumbnail_url?: string;
  title?: string;
  /** Empty when it isn't live. */
  started_at?: string;
};

/** `GET /users`. */
export type HelixUser = {
  id: string;
  login: string;
  display_name: string;
  description?: string;
  profile_image_url?: string;
  offline_image_url?: string;
};

/** `GET /channels`. */
export type HelixChannel = {
  broadcaster_id: string;
  broadcaster_login?: string;
  broadcaster_name?: string;
  game_name?: string;
  title?: string;
  tags?: string[] | null;
};

/** `GET /videos`: a past broadcast, a highlight or an upload. */
export type HelixVideo = {
  id: string;
  user_login?: string;
  title?: string;
  description?: string;
  created_at?: string;
  published_at?: string;
  /** `…%{width}x%{height}…`; empty while a broadcast is being processed. */
  thumbnail_url?: string;
  view_count?: number;
  type?: 'archive' | 'highlight' | 'upload' | string;
  /** "3h8m33s". */
  duration?: string;
};

/** `GET /clips`. */
export type HelixClip = {
  id: string;
  broadcaster_name?: string;
  title?: string;
  created_at?: string;
  view_count?: number;
  /** Seconds. */
  duration?: number;
};

/** `GET /games/top`. */
export type HelixGame = { id: string; name: string };

/** A Helix list: its items and the cursor of the next page. */
export type HelixPage<T> = { data?: T[]; pagination?: { cursor?: string } };

const WEBSITE = 'https://www.twitch.tv';

/** Unix ms of an ISO time; null for none or an unreadable one. */
export const timeOf = (iso: string | undefined | null): number | null => {
  if (!iso) return null;
  const ms = Date.parse(iso);
  return Number.isFinite(ms) && ms > 0 ? ms : null;
};

/** A picture address with its size filled in (`{width}x{height}` or `%{width}x%{height}`). */
export const sized = (
  template: string | undefined | null,
  width: number,
  height: number,
): string | null => {
  if (!template || !/^https:\/\//.test(template)) return null;
  return template.replace(/%?\{width\}/g, String(width)).replace(/%?\{height\}/g, String(height));
};

/**
 * A live preview's address. Twitch draws a new one every few minutes under the same address; a
 * ten-minute mark in it makes Raygun's cover cache fetch it again.
 */
export const preview = (template: string | undefined | null, now: number): string | null => {
  const url = sized(template, 640, 360);
  return url && `${url}?t=${Math.floor(now / 600_000)}`;
};

/** "3h8m33s" as milliseconds. */
export const durationMs = (text: string | undefined | null): number | null => {
  const found = /^(?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s)?$/.exec((text ?? '').trim());
  if (!found || !found[0]) return null;
  const [, hours = '0', minutes = '0', seconds = '0'] = found;
  return ((Number(hours) * 60 + Number(minutes)) * 60 + Number(seconds)) * 1000;
};

/** A login as Twitch spells it: 1–25 letters, digits or underscores. */
export const isLogin = (text: string): boolean => /^[a-z0-9_]{1,25}$/i.test(text);

const line = (...parts: (string | undefined | null)[]) =>
  parts
    .map((part) => (part ?? '').trim())
    .filter(Boolean)
    .join(' · ') || null;

/** A stream on the air as a listed title: the channel, its stream's title on the cover. */
export const streamItem = (stream: HelixStream, now: number): MangaSummary => ({
  id: stream.user_login.toLowerCase(),
  title: stream.title?.trim() || stream.user_name,
  coverUrl: preview(stream.thumbnail_url, now),
  subtitle: line(stream.user_name, stream.game_name),
  live: true,
  viewers: typeof stream.viewer_count === 'number' ? stream.viewer_count : null,
  startedAt: timeOf(stream.started_at),
});

/** A channel a search found: live ones say what they stream. */
export const channelItem = (channel: HelixChannelResult): MangaSummary => {
  const live = channel.is_live === true;
  return {
    id: channel.broadcaster_login.toLowerCase(),
    title: channel.display_name || channel.broadcaster_login,
    coverUrl: sized(channel.thumbnail_url, 300, 300),
    subtitle: live ? line(channel.title, channel.game_name) : line(channel.game_name),
    live,
    startedAt: live ? timeOf(channel.started_at) : null,
  };
};

/** How many people watch, as a short English number: "12.3K". */
export const viewerCount = (count: number): string => {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1).replace(/\.0$/, '')}K`;
  return String(count);
};

/** A channel's page: its stream when it's live, else its offline picture. */
export const channelDetails = (
  user: HelixUser,
  channel: HelixChannel | null,
  stream: HelixStream | null,
  now: number,
): VideoDetails => {
  const live =
    stream &&
    line(
      `Live now: ${stream.title?.trim() || 'untitled stream'}`,
      stream.game_name,
      typeof stream.viewer_count === 'number'
        ? `${viewerCount(stream.viewer_count)} watching`
        : null,
    );
  return {
    id: user.login,
    title: user.display_name || user.login,
    coverUrl:
      (stream && preview(stream.thumbnail_url, now)) ||
      sized(user.offline_image_url, 1920, 1080) ||
      sized(user.profile_image_url, 300, 300),
    description: [live, user.description?.trim()].filter(Boolean).join('\n\n') || null,
    genres: [channel?.game_name, ...(channel?.tags ?? [])]
      .filter((genre): genre is string => Boolean(genre && genre.trim()))
      .slice(0, 10),
    type: 'other',
    status: stream ? 'ongoing' : 'unknown',
    url: `${WEBSITE}/${user.login}`,
  };
};

/** Episode ids: a channel's stream, one of its videos, one of its clips. */
export const LIVE = 'live:';
export const VIDEO = 'video:';
export const CLIP = 'clip:';

export const liveEpisode = (login: string, stream: HelixStream): VideoEpisode => ({
  id: `${LIVE}${login}`,
  name: `Live now: ${stream.title?.trim() || 'untitled stream'}`,
  dateUpload: timeOf(stream.started_at),
});

const VIDEO_KINDS: Record<string, string> = { highlight: 'Highlight', upload: 'Upload' };

export const videoEpisode = (video: HelixVideo): VideoEpisode => {
  const kind = VIDEO_KINDS[video.type ?? ''];
  const title = video.title?.trim() || 'Past broadcast';
  return {
    id: `${VIDEO}${video.id}`,
    name: kind ? `${kind}: ${title}` : title,
    dateUpload: timeOf(video.published_at) ?? timeOf(video.created_at),
  };
};

export const clipEpisode = (clip: HelixClip): VideoEpisode => ({
  id: `${CLIP}${clip.id}`,
  name: `Clip: ${clip.title?.trim() || clip.id}`,
  dateUpload: timeOf(clip.created_at),
});

/**
 * Where an episode plays: Twitch's own player, as Twitch's developer agreement requires for all
 * Twitch video, opened by Raygun in its browser (`format: 'page'`): Raygun's window can't frame it
 * (Twitch's embeds need an https parent page). First Twitch's page for it, which has the chat,
 * then the player alone (the address of Twitch's own pop-out player).
 */
export const playerPages = (episodeId: string): Video[] => {
  const page = (url: string, label: string, live = false): Video => ({
    url,
    label,
    format: 'page',
    ...(live ? { live: true } : {}),
  });
  if (episodeId.startsWith(LIVE)) {
    const channel = episodeId.slice(LIVE.length);
    if (!isLogin(channel)) return [];
    return [
      page(`${WEBSITE}/${channel}`, 'On Twitch, with chat', true),
      page(
        `https://player.twitch.tv/?channel=${channel}&parent=twitch.tv&player=popout`,
        "Twitch's player alone",
        true,
      ),
    ];
  }
  if (episodeId.startsWith(VIDEO)) {
    const id = episodeId.slice(VIDEO.length);
    if (!/^\d+$/.test(id)) return [];
    return [
      page(`${WEBSITE}/videos/${id}`, 'On Twitch, with chat'),
      page(
        `https://player.twitch.tv/?video=v${id}&parent=twitch.tv&player=popout`,
        "Twitch's player alone",
      ),
    ];
  }
  if (episodeId.startsWith(CLIP)) {
    const slug = episodeId.slice(CLIP.length);
    if (!/^[\w-]{1,120}$/.test(slug)) return [];
    return [page(`https://clips.twitch.tv/${slug}`, 'On Twitch')];
  }
  return [];
};

/** The categories Twitch's top games are, for the Category filter. */
export const categoryOptions = (games: HelixGame[]) =>
  games
    .filter((game) => /^\d+$/.test(game.id) && game.name.trim())
    .map((game) => ({ value: game.id, label: game.name.trim() }));

/** Stream languages Twitch lists most, by their ISO 639-1 codes. */
export const LANGUAGES: [string, string][] = [
  ['en', 'English'],
  ['es', 'Spanish'],
  ['pt', 'Portuguese'],
  ['de', 'German'],
  ['fr', 'French'],
  ['ru', 'Russian'],
  ['ja', 'Japanese'],
  ['ko', 'Korean'],
  ['zh', 'Chinese'],
  ['it', 'Italian'],
  ['pl', 'Polish'],
  ['tr', 'Turkish'],
  ['ar', 'Arabic'],
  ['nl', 'Dutch'],
  ['sv', 'Swedish'],
  ['uk', 'Ukrainian'],
  ['cs', 'Czech'],
  ['hu', 'Hungarian'],
  ['th', 'Thai'],
  ['id', 'Indonesian'],
  ['vi', 'Vietnamese'],
  ['fi', 'Finnish'],
  ['da', 'Danish'],
  ['no', 'Norwegian'],
  ['el', 'Greek'],
  ['ro', 'Romanian'],
];
