import type { MusicAlbum, ReleaseKind } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const MUSICBRAINZ = {
  id: 'dev.raygun.musicbrainz/recordings',
  extensionId: 'dev.raygun.musicbrainz',
  kind: 'music.metadata',
  name: 'MusicBrainz',
};

const release = (id: string, title: string, year: number, kind: ReleaseKind): MusicAlbum => ({
  provider: MUSICBRAINZ,
  id,
  title,
  artist: 'Nina Simone',
  year,
  artworkUrl: null,
  trackCount: null,
  kind,
});

/** A route as Rust builds it for a release's notification (`follows::release_route`). */
const query = (search: Record<string, string>) => new URLSearchParams(search).toString();

describe("A followed artist's new release, tapped in the notifications", () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    localStorage.clear();
  });

  it("opens a new album over its artist's page", async () => {
    const user = userEvent.setup();
    const album = release('a1', 'Little Girl Blue', 2026, 'album');
    commands.musicAlbum.mockResolvedValue(ok({ album, tracks: [] }));
    commands.musicArtist.mockResolvedValue(
      ok({ name: 'Nina Simone', popular: [], albums: [], tracks: [] }),
    );
    // Tapped while Raygun was closed: the route waited in Rust until the UI asked.
    commands.notificationRouteTake.mockResolvedValueOnce(
      `/artist?${query({ name: 'Nina Simone' })}\n/music/album?${query({ provider: MUSICBRAINZ.id, id: 'a1' })}`,
    );
    await AppShellWrapper.mount();

    const page = await screen.findByTestId('music-album-page');
    expect(await within(page).findByRole('heading', { level: 1 })).toHaveTextContent(
      'Little Girl Blue',
    );
    expect(commands.musicAlbum).toHaveBeenCalledWith(MUSICBRAINZ.id, 'a1');

    // Back leads to the artist.
    await user.click(within(page).getByRole('button', { name: 'Back' }));
    const artist = await screen.findByTestId('music-artist-page');
    expect(within(artist).getByRole('heading', { name: 'Nina Simone' })).toBeInTheDocument();
  });

  it("marks a new single in its artist's discography, wherever it is", async () => {
    // A long discography, oldest first: the single is past Show all.
    localStorage.setItem('raygun.discography.sort', JSON.stringify({ all: 'oldest' }));
    const albums = Array.from({ length: 14 }, (_, index) =>
      release(`a${index}`, `Album ${index}`, 1958 + index, 'album'),
    );
    commands.musicArtistReleases.mockResolvedValue(
      ok([...albums, release('s1', 'Feeling Good (Live)', 2026, 'single')]),
    );
    await AppShellWrapper.mount();

    // Tapped while Raygun runs.
    emitEvent(
      'openRouteEvent',
      `/artist?${query({ name: 'Nina Simone', reveal: `release:${MUSICBRAINZ.id}/s1` })}`,
    );

    const page = await screen.findByTestId('music-artist-page');
    const discography = await within(page).findByTestId('discography');
    await waitFor(() =>
      expect(discography.querySelector('[data-revealed]')).toHaveTextContent('Feeling Good (Live)'),
    );
    expect(within(discography).getByText('Feeling Good (Live)')).toBeInTheDocument();
    expect(commands.notificationRouteTake).toHaveBeenCalled();
  });
});
