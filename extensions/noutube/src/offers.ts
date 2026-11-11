// What the page offers to Raygun (raygunPage.offer): the video or song of a watch page, and the
// playlist it plays from. YouTube moves between pages without loading them, so the offers follow
// its navigation events, the player, and once a second the address.

import type { PageOffer } from '@raygun/sdk/site';

import { isObject, pick, pickString, textOf } from './lib/json';
import { isMusic, onNavigate, watchVideoId } from './page';
import {
  getPlayer,
  onPlayerState,
  onVideoChange,
  playerVideoId,
  readVideo,
  type Player,
} from './player';

const LIST_ID = /^[A-Za-z0-9_-]+$/;
// Lists only their owner can open (Watch later, Liked videos, YouTube Music's liked songs) and
// mixes, which YouTube makes for each viewer and gives no playlist page.
const PERSONAL_LISTS = new Set(['WL', 'LL', 'LM']);
const isPersonalList = (id: string) => PERSONAL_LISTS.has(id) || id.startsWith('RD');

const SITE = isMusic ? 'https://music.youtube.com' : 'https://www.youtube.com';

/** The address the document was loaded with: the page's own data and tags describe it. */
const loadedHref = location.href;

type VideoFacts = {
  title?: string;
  artist?: string;
  album?: string;
  artworkUrl?: string;
  seconds?: number;
  live?: boolean;
};

const nonEmpty = (text: string | undefined | null) => (text ? text.trim() || undefined : undefined);

/** YouTube's channels for an artist's songs are named "<artist> - Topic". */
const artistName = (author: string | undefined) => nonEmpty(author?.replace(/ - Topic$/, ''));

function fromPlayer(player: Player | undefined, id: string): VideoFacts {
  if (!player || playerVideoId(player) !== id) return {};
  const facts: VideoFacts = {};
  try {
    const data = player.getVideoData?.();
    facts.title = nonEmpty(pickString(data, 'title'));
    facts.artist = nonEmpty(pickString(data, 'author'));
    facts.live = pick(data, 'isLive') === true;
    const duration = player.getDuration?.();
    if (typeof duration === 'number' && Number.isFinite(duration) && duration > 0) {
      facts.seconds = duration;
    }
  } catch {
    // The player's API can be half there while it loads.
  }
  const video = readVideo(player);
  if (video?.id === id) {
    facts.title ??= nonEmpty(video.title);
    facts.artist ??= nonEmpty(video.author);
    facts.live ||= video.live;
    if (!facts.seconds && video.lengthSeconds > 0) facts.seconds = video.lengthSeconds;
  }
  return facts;
}

/** The first page's own player answer and tags, while they are about this video. */
function fromPage(id: string): VideoFacts {
  const details = pick(window.ytInitialPlayerResponse, 'videoDetails');
  if (pickString(details, 'videoId') === id) {
    return {
      title: nonEmpty(pickString(details, 'title')),
      artist: nonEmpty(pickString(details, 'author')),
      seconds: Number(pick(details, 'lengthSeconds')) || undefined,
      live: pick(details, 'isLive') === true,
    };
  }
  if (watchVideoId(loadedHref) !== id) return {};
  const meta = (selector: string) =>
    nonEmpty(document.querySelector(selector)?.getAttribute('content'));
  return {
    title: meta('meta[name="title"]') ?? meta('meta[property="og:title"]'),
    artist: meta('[itemprop="author"] [itemprop="name"]'),
  };
}

/** What YouTube Music tells the system of the song: its artist, album and cover. */
function fromMediaSession(title: string | undefined): VideoFacts {
  const metadata = navigator.mediaSession?.metadata;
  if (!metadata || !title || metadata.title !== title) return {};
  const artwork = [...metadata.artwork]
    .reverse()
    .find((picture) => picture.src.startsWith('https://'));
  return {
    artist: nonEmpty(metadata.artist),
    album: nonEmpty(metadata.album),
    artworkUrl: artwork?.src,
  };
}

function videoOffer(id: string): PageOffer {
  const player = getPlayer();
  const playerFacts = fromPlayer(player, id);
  const pageFacts = fromPage(id);
  const title = playerFacts.title ?? pageFacts.title;
  const sessionFacts = isMusic ? fromMediaSession(title) : {};
  const author = sessionFacts.artist ?? playerFacts.artist ?? pageFacts.artist;
  const live = playerFacts.live || pageFacts.live;
  const seconds = live ? undefined : (playerFacts.seconds ?? pageFacts.seconds);

  const offer: PageOffer = {
    kind: isMusic ? 'song' : 'video',
    url: `${SITE}/watch?v=${id}`,
    artworkUrl: sessionFacts.artworkUrl ?? `https://i.ytimg.com/vi/${id}/hqdefault.jpg`,
  };
  if (title) offer.title = title;
  const artist = isMusic ? artistName(author) : author;
  if (artist) offer.artist = artist;
  if (sessionFacts.album) offer.album = sessionFacts.album;
  if (seconds && seconds > 0) offer.durationMs = Math.round(seconds) * 1000;
  return offer;
}

/** A playlist's title among a watch page's answers: the desktop site's, the mobile one's, YouTube Music's queue. */
function playlistTitleIn(response: unknown, listId: string): string | undefined {
  const candidates = [
    pick(response, 'contents', 'twoColumnWatchNextResults', 'playlist', 'playlist'),
    pick(response, 'contents', 'singleColumnWatchNextResults', 'playlist', 'playlist'),
    pick(
      response,
      'contents',
      'singleColumnMusicWatchNextResultsRenderer',
      'tabbedRenderer',
      'watchNextTabbedResultsRenderer',
      'tabs',
      0,
      'tabRenderer',
      'content',
      'musicQueueRenderer',
      'content',
      'playlistPanelRenderer',
    ),
  ];
  for (const playlist of candidates) {
    if (!isObject(playlist) || playlist.playlistId !== listId) continue;
    const title = nonEmpty(textOf(playlist.title));
    if (title) return title;
  }
  return undefined;
}

/** A playlist page's own title, from the first page's data while it is about this list. */
function playlistPageTitle(listId: string): string | undefined {
  const data = window.ytInitialData;
  const canonical = pickString(data, 'microformat', 'microformatDataRenderer', 'urlCanonical');
  if (!canonical?.includes(`list=${listId}`)) return undefined;
  return (
    nonEmpty(pickString(data, 'metadata', 'playlistMetadataRenderer', 'title')) ??
    nonEmpty(pickString(data, 'microformat', 'microformatDataRenderer', 'title'))
  );
}

const playlistTitles = new Map<string, string>();

function playlistOffer(listId: string): PageOffer {
  let title = playlistTitles.get(listId);
  if (!title) {
    let response: unknown;
    try {
      response = getPlayer()?.getWatchNextResponse?.();
    } catch {
      response = undefined;
    }
    title =
      playlistTitleIn(response, listId) ??
      playlistTitleIn(window.ytInitialData, listId) ??
      playlistPageTitle(listId);
    if (title) playlistTitles.set(listId, title);
  }
  const offer: PageOffer = { kind: 'playlist', url: `${SITE}/playlist?list=${listId}` };
  if (title) offer.title = title;
  return offer;
}

function currentOffers(): PageOffer[] {
  const offers: PageOffer[] = [];
  const url = new URL(location.href);
  // Only an id of YouTube's form ([A-Za-z0-9_-]{11}); see watchVideoId.
  const videoId = watchVideoId();
  if (videoId) offers.push(videoOffer(videoId));
  const listId = url.searchParams.get('list') ?? '';
  const listPage = url.pathname === '/watch' || url.pathname === '/playlist';
  if (listPage && LIST_ID.test(listId) && !isPersonalList(listId)) {
    offers.push(playlistOffer(listId));
  }
  return offers;
}

let offered = '';

function refresh() {
  let offers: PageOffer[];
  try {
    offers = currentOffers();
  } catch (error) {
    console.error('[YouTube enhancements] offers:', error);
    return;
  }
  const key = JSON.stringify(offers);
  if (key === offered) return;
  // The page's previous offers go first: what it offers now is the whole of it.
  if (offered) raygunPage.withdraw();
  offered = key;
  for (const offer of offers) raygunPage.offer(offer);
}

export function installOffers() {
  refresh();
  onNavigate(refresh);
  onVideoChange(() => refresh());
  onPlayerState(() => refresh());
  // Some moves (YouTube Music's next song, an address changed in place) fire none of the above.
  setInterval(refresh, 1000);
}
