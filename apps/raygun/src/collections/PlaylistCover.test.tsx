import type { TrackQuery } from '@raygun/bindings';
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';

import { bindingsMock } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { PlaylistCover } from './PlaylistCover';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const URLS = [1, 2, 3, 4].map((number) => `http://127.0.0.1:9/cover/${number}`);
const SONGS: TrackQuery[] = ['One', 'Two', 'Three', 'Four'].map((title) => ({
  title,
  artist: 'Artist',
  album: null,
  durationMs: null,
}));

const tiles = () => within(screen.getByTestId('playlist-mosaic')).queryAllByTestId('artwork-image');

const tile = (index: number) => {
  const found = tiles()[index];
  if (!found) throw new Error(`No picture ${index} in the cover`);
  return found;
};

describe('a playlist cover made of four songs', () => {
  beforeEach(() => {
    commands.coverFallback.mockReset();
    commands.coverFallback.mockResolvedValue(ok(null));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('loads its four pictures at once, without waiting for the browser', () => {
    renderWithProviders(<PlaylistCover artworkUrls={URLS} artworkSongs={SONGS} />);
    expect(tiles().map((picture) => picture.getAttribute('src'))).toEqual(URLS);
    for (const picture of tiles()) expect(picture).toHaveAttribute('loading', 'eager');
  });

  it("shows another source's cover of a song whose own picture fails", async () => {
    const found = 'http://127.0.0.1:9/cover/found';
    commands.coverFallback.mockResolvedValue(ok(found));
    renderWithProviders(<PlaylistCover artworkUrls={URLS} artworkSongs={SONGS} />);
    fireEvent.error(tile(1));
    await waitFor(() =>
      expect(commands.coverFallback).toHaveBeenCalledWith({ kind: 'song', query: SONGS[1] }),
    );
    await waitFor(() => expect(tile(1)).toHaveAttribute('src', found));
    // The others keep theirs, and nothing else was asked for.
    expect(tile(0)).toHaveAttribute('src', URLS[0]);
    expect(commands.coverFallback).toHaveBeenCalledTimes(1);
  });

  it('a picture failing without its song known shows the placeholder', () => {
    renderWithProviders(<PlaylistCover artworkUrls={URLS} />);
    fireEvent.error(tile(2));
    expect(tiles()).toHaveLength(3);
    expect(commands.coverFallback).not.toHaveBeenCalled();
  });

  it('waits until it comes near the screen, then asks for all four together', () => {
    let report: (entries: Partial<IntersectionObserverEntry>[]) => void = () => {};
    const observed: Element[] = [];
    vi.stubGlobal(
      'IntersectionObserver',
      class {
        constructor(callback: (entries: Partial<IntersectionObserverEntry>[]) => void) {
          report = callback;
        }
        observe(element: Element) {
          observed.push(element);
        }
        unobserve() {}
        disconnect() {}
      },
    );
    renderWithProviders(<PlaylistCover artworkUrls={URLS} artworkSongs={SONGS} />);
    const mosaic = screen.getByTestId('playlist-mosaic');
    expect(observed).toEqual([mosaic]);
    expect(tiles()).toHaveLength(0);

    act(() => report([{ target: mosaic, isIntersecting: true }]));
    expect(tiles()).toHaveLength(4);
    for (const picture of tiles()) expect(picture).toHaveAttribute('loading', 'eager');
  });

  it("falls back for the first song when there aren't four pictures", async () => {
    const found = 'http://127.0.0.1:9/cover/found';
    commands.coverFallback.mockResolvedValue(ok(found));
    renderWithProviders(
      <PlaylistCover artworkUrls={URLS.slice(0, 2)} artworkSongs={SONGS.slice(0, 2)} />,
    );
    const picture = screen.getByTestId('artwork-image');
    expect(picture).toHaveAttribute('src', URLS[0]);
    fireEvent.error(picture);
    await waitFor(() =>
      expect(commands.coverFallback).toHaveBeenCalledWith({ kind: 'song', query: SONGS[0] }),
    );
    await waitFor(() => expect(screen.getByTestId('artwork-image')).toHaveAttribute('src', found));
  });
});
