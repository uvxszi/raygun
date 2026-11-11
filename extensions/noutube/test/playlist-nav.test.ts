// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From lib/playlist-nav.test.ts.

import { describe, expect, it } from 'vitest';

import {
  getHeldPlaylist,
  getPlayingPlaylist,
  getPlaylistNeighborUrl,
} from '../src/lib/playlist-nav';

const entry = (videoId: string, selected = false) => ({
  playlistPanelVideoRenderer: { videoId, selected },
});
const button = (videoId: string, url?: string) => ({
  buttonRenderer: {
    command: {
      commandMetadata: { webCommandMetadata: { url } },
      watchEndpoint: { videoId, playlistId: 'PL1' },
    },
  },
});

const data = {
  playlistId: 'PL1',
  contents: [entry('aaa'), entry('bbb', true), entry('ccc')],
  previousButtonVideo: button('aaa', '/watch?v=aaa&list=PL1&index=1'),
  nextButtonVideo: button('ccc', '/watch?v=ccc&list=PL1&index=3'),
};

const wrap = (playlist: unknown) => ({
  contents: { singleColumnWatchNextResults: { playlist: { playlist } } },
});

describe('getPlaylistNeighborUrl', () => {
  for (const layout of ['singleColumnWatchNextResults', 'twoColumnWatchNextResults']) {
    it(`recognizes the playing playlist in ${layout}`, () => {
      const response = { contents: { [layout]: { playlist: { playlist: data } } } };
      expect(getPlayingPlaylist(response, 'bbb')).toBe(data);
      expect(getPlayingPlaylist(response, 'zzz')).toBeUndefined();
      expect(getPlayingPlaylist(response)).toBeUndefined();
    });
  }

  it('keeps an exhausted playlist active so media controls do not leave it', () => {
    const playlist = { contents: [entry('aaa', true)], playlistId: 'PL1' };
    expect(getPlayingPlaylist(wrap(playlist), 'aaa')).toBe(playlist);
    expect(getPlaylistNeighborUrl(playlist, 'next', 'aaa')).toBeUndefined();
    expect(getPlaylistNeighborUrl(playlist, 'previous', 'aaa')).toBeUndefined();
  });

  it('finds the playing video in a playlist that was fetched for an earlier one', () => {
    const stale = wrap(data);
    expect(getPlayingPlaylist(stale, 'aaa')).toBeUndefined();
    expect(getHeldPlaylist([stale], 'aaa', 'PL1')).toBe(data);
    // The skip buttons still skip from the selected video, so they are no help.
    expect(getPlaylistNeighborUrl(data, 'next', 'aaa')).toBe('/watch?v=bbb&list=PL1');
    expect(getPlaylistNeighborUrl(data, 'previous', 'ccc')).toBe('/watch?v=bbb&list=PL1');
    expect(getPlaylistNeighborUrl(data, 'previous', 'aaa')).toBeUndefined();
    expect(getPlaylistNeighborUrl(data, 'next', 'ccc')).toBeUndefined();
  });

  it('prefers the held response that is about the playing video', () => {
    const current = {
      playlistId: 'PL1',
      contents: [entry('aaa', true), entry('bbb'), entry('ccc')],
    };
    expect(getHeldPlaylist([wrap(data), wrap(current)], 'aaa', 'PL1')).toBe(current);
    expect(getHeldPlaylist([undefined, wrap(current)], 'aaa', 'PL1')).toBe(current);
  });

  it('drops a held playlist once the page left it', () => {
    const held = [wrap(data)];
    expect(getHeldPlaylist(held, 'bbb', null)).toBeUndefined();
    expect(getHeldPlaylist(held, 'bbb', 'PL2')).toBeUndefined();
    expect(getHeldPlaylist(held, 'zzz', 'PL1')).toBeUndefined();
  });

  it('follows the playlist skip buttons', () => {
    expect(getPlaylistNeighborUrl(data, 'next', 'bbb')).toBe('/watch?v=ccc&list=PL1&index=3');
    expect(getPlaylistNeighborUrl(data, 'previous', 'bbb')).toBe('/watch?v=aaa&list=PL1&index=1');
  });

  it('falls back to the entries around the selected one', () => {
    const { contents, playlistId } = data;
    expect(getPlaylistNeighborUrl({ contents, playlistId }, 'next')).toBe('/watch?v=ccc&list=PL1');
    expect(getPlaylistNeighborUrl({ contents, playlistId }, 'previous')).toBe(
      '/watch?v=aaa&list=PL1',
    );
  });

  it('ignores a playlist that is about another video', () => {
    expect(getPlaylistNeighborUrl(data, 'next', 'zzz')).toBeUndefined();
    expect(getPlaylistNeighborUrl(undefined, 'next', 'bbb')).toBeUndefined();
  });

  it('has nothing past the ends of the playlist', () => {
    const contents = [entry('aaa', true), entry('bbb')];
    expect(
      getPlaylistNeighborUrl({ contents, previousButtonVideo: button('aaa') }, 'previous', 'aaa'),
    ).toBeUndefined();
    expect(
      getPlaylistNeighborUrl({ contents: [entry('aaa'), entry('bbb', true)] }, 'next', 'bbb'),
    ).toBeUndefined();
  });
});
