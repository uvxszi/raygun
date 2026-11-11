import { describe, expect, test } from 'vitest';

import {
  categoryOptions,
  channelDetails,
  channelItem,
  clipEpisode,
  durationMs,
  liveEpisode,
  playerPages,
  preview,
  sized,
  streamItem,
  timeOf,
  videoEpisode,
  viewerCount,
} from '../src/parse';
import type {
  HelixChannel,
  HelixChannelResult,
  HelixClip,
  HelixStream,
  HelixUser,
  HelixVideo,
} from '../src/parse';

// Shapes from Helix's reference answers (dev.twitch.tv/docs/api/reference).
const STREAM: HelixStream = {
  id: '40952121085',
  user_id: '101051819',
  user_login: 'AfroCHESS',
  user_name: 'AfroChess',
  game_id: '743',
  game_name: 'Chess',
  type: 'live',
  title: ' Blitz with viewers ',
  tags: ['English'],
  viewer_count: 12_345,
  started_at: '2026-11-11T18:00:00Z',
  language: 'en',
  thumbnail_url:
    'https://static-cdn.jtvnw.net/previews-ttv/live_user_afrochess-{width}x{height}.jpg',
};

const CHANNEL_RESULT: HelixChannelResult = {
  id: '101051819',
  broadcaster_login: 'afrochess',
  display_name: 'AfroChess',
  broadcaster_language: 'en',
  game_id: '743',
  game_name: 'Chess',
  is_live: true,
  tags: [],
  thumbnail_url:
    'https://static-cdn.jtvnw.net/jtv_user_pictures/afrochess-profile_image-300x300.png',
  title: 'Blitz with viewers',
  started_at: '2026-11-11T18:00:00Z',
};

const USER: HelixUser = {
  id: '101051819',
  login: 'afrochess',
  display_name: 'AfroChess',
  description: 'Chess every evening.',
  profile_image_url:
    'https://static-cdn.jtvnw.net/jtv_user_pictures/afrochess-profile_image-300x300.png',
  offline_image_url:
    'https://static-cdn.jtvnw.net/jtv_user_pictures/afrochess-offline-1920x1080.png',
};

const CHANNEL: HelixChannel = {
  broadcaster_id: '101051819',
  game_name: 'Chess',
  title: 'Blitz with viewers',
  tags: ['English', 'Educational'],
};

const NOW = Date.parse('2026-11-11T19:05:00Z');

describe('listed streams and channels', () => {
  test('a live stream is its channel, with the stream on the cover and its viewers', () => {
    expect(streamItem(STREAM, NOW)).toEqual({
      id: 'afrochess',
      title: 'Blitz with viewers',
      coverUrl: `https://static-cdn.jtvnw.net/previews-ttv/live_user_afrochess-640x360.jpg?t=${Math.floor(NOW / 600_000)}`,
      subtitle: 'AfroChess · Chess',
      live: true,
      viewers: 12_345,
      startedAt: Date.parse('2026-11-11T18:00:00Z'),
    });
  });

  test('an untitled stream is called by its channel', () => {
    expect(streamItem({ ...STREAM, title: '  ' }, NOW).title).toBe('AfroChess');
  });

  test('a channel a search found says what it streams while live', () => {
    expect(channelItem(CHANNEL_RESULT)).toMatchObject({
      id: 'afrochess',
      title: 'AfroChess',
      subtitle: 'Blitz with viewers · Chess',
      live: true,
    });
    const offline = channelItem({ ...CHANNEL_RESULT, is_live: false, started_at: '' });
    expect(offline).toMatchObject({ live: false, subtitle: 'Chess', startedAt: null });
  });

  test('previews change address every ten minutes, so they are fetched again', () => {
    const at = preview(STREAM.thumbnail_url, NOW);
    expect(preview(STREAM.thumbnail_url, NOW + 60_000)).toBe(at);
    expect(preview(STREAM.thumbnail_url, NOW + 600_000)).not.toBe(at);
    expect(preview('http://insecure.example/{width}x{height}.jpg', NOW)).toBeNull();
  });
});

describe('a channel page', () => {
  test('live: the stream on the cover and in the description', () => {
    const details = channelDetails(USER, CHANNEL, STREAM, NOW);
    expect(details).toMatchObject({
      id: 'afrochess',
      title: 'AfroChess',
      genres: ['Chess', 'English', 'Educational'],
      type: 'other',
      status: 'ongoing',
      url: 'https://www.twitch.tv/afrochess',
    });
    expect(details.coverUrl).toContain('/previews-ttv/live_user_afrochess-640x360.jpg');
    expect(details.description).toBe(
      'Live now: Blitz with viewers · Chess · 12.3K watching\n\nChess every evening.',
    );
  });

  test('offline: its offline picture, then its profile picture', () => {
    expect(channelDetails(USER, null, null, NOW)).toMatchObject({
      coverUrl: USER.offline_image_url,
      description: 'Chess every evening.',
      genres: [],
      status: 'unknown',
    });
    const plain = channelDetails({ ...USER, offline_image_url: '' }, null, null, NOW);
    expect(plain.coverUrl).toBe(USER.profile_image_url);
  });
});

describe('episodes', () => {
  test('the stream on the air, then videos and clips named by their kind', () => {
    expect(liveEpisode('afrochess', STREAM)).toEqual({
      id: 'live:afrochess',
      name: 'Live now: Blitz with viewers',
      dateUpload: Date.parse('2026-11-11T18:00:00Z'),
    });
    const archive: HelixVideo = {
      id: '335921245',
      title: 'Sunday blitz',
      created_at: '2026-11-08T17:00:00Z',
      published_at: '2026-11-08T17:00:05Z',
      type: 'archive',
      duration: '3h8m33s',
    };
    expect(videoEpisode(archive)).toEqual({
      id: 'video:335921245',
      name: 'Sunday blitz',
      dateUpload: Date.parse('2026-11-08T17:00:05Z'),
    });
    expect(videoEpisode({ ...archive, type: 'highlight' }).name).toBe('Highlight: Sunday blitz');
    expect(videoEpisode({ ...archive, title: '' }).name).toBe('Past broadcast');
    const clip: HelixClip = {
      id: 'AwkwardHelplessSalamanderSwiftRage',
      title: 'Queen sacrifice',
      created_at: '2026-11-01T20:00:00Z',
    };
    expect(clipEpisode(clip)).toEqual({
      id: 'clip:AwkwardHelplessSalamanderSwiftRage',
      name: 'Clip: Queen sacrifice',
      dateUpload: Date.parse('2026-11-01T20:00:00Z'),
    });
  });

  test("everything plays in Twitch's own player, opened as a page", () => {
    expect(playerPages('live:afrochess')).toEqual([
      {
        url: 'https://www.twitch.tv/afrochess',
        label: 'On Twitch, with chat',
        format: 'page',
        live: true,
      },
      {
        url: 'https://player.twitch.tv/?channel=afrochess&parent=twitch.tv&player=popout',
        label: "Twitch's player alone",
        format: 'page',
        live: true,
      },
    ]);
    expect(playerPages('video:335921245').map((video) => video.url)).toEqual([
      'https://www.twitch.tv/videos/335921245',
      'https://player.twitch.tv/?video=v335921245&parent=twitch.tv&player=popout',
    ]);
    expect(playerPages('clip:AwkwardHelplessSalamanderSwiftRage')).toEqual([
      {
        url: 'https://clips.twitch.tv/AwkwardHelplessSalamanderSwiftRage',
        label: 'On Twitch',
        format: 'page',
      },
    ]);
    expect(playerPages('live:not a login')).toEqual([]);
    expect(playerPages('video:abc')).toEqual([]);
    expect(playerPages('something:else')).toEqual([]);
  });
});

describe('small readings', () => {
  test('durations, sizes, times and counts', () => {
    expect(durationMs('3h8m33s')).toBe(11_313_000);
    expect(durationMs('45s')).toBe(45_000);
    expect(durationMs('')).toBeNull();
    expect(sized('https://img.example/%{width}x%{height}.jpg', 320, 180)).toBe(
      'https://img.example/320x180.jpg',
    );
    expect(timeOf('not a time')).toBeNull();
    expect(viewerCount(999)).toBe('999');
    expect(viewerCount(12_000)).toBe('12K');
    expect(viewerCount(1_250_000)).toBe('1.3M');
    expect(
      categoryOptions([
        { id: '509658', name: 'Just Chatting' },
        { id: 'x', name: 'Bad id' },
        { id: '743', name: ' ' },
      ]),
    ).toEqual([{ value: '509658', label: 'Just Chatting' }]);
  });
});
