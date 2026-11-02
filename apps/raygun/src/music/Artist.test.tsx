import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { creditedArtists } from './artists';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const song = (title: string, artist: string) => ({
  externalId: null,
  title,
  artist,
  album: 'Random Access Memories',
  durationMs: 200_000,
  artworkUrl: null,
  source: null,
});

describe('Artists', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('splits credits into artists', () => {
    expect(creditedArtists('Daft Punk, Pharrell Williams & Nile Rodgers')).toEqual([
      'Daft Punk',
      'Pharrell Williams',
      'Nile Rodgers',
    ]);
    expect(creditedArtists('Calvin Harris feat. Rihanna')).toEqual(['Calvin Harris', 'Rihanna']);
    expect(creditedArtists(null)).toEqual([]);
  });

  it('leads from a credit to the artist page, with popular songs to play', async () => {
    const user = userEvent.setup();
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: {
              id: 'x/y',
              extensionId: 'x',
              kind: 'music.metadata',
              name: 'Source',
            },
            tracks: [song('Get Lucky', 'Daft Punk, Pharrell Williams')],
            error: null,
          },
        ],
      }),
    );
    commands.musicArtist.mockResolvedValue(
      ok({
        name: 'Pharrell Williams',
        popular: [
          song('Happy', 'Pharrell Williams'),
          song('Get Lucky', 'Daft Punk, Pharrell Williams'),
        ],
        albums: [],
        tracks: [],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=get+lucky');

    // The second artist of the credit is a link of its own; it doesn't play the row.
    const row = await screen.findByTestId('online-track-row');
    await user.click(within(row).getByRole('link', { name: 'Pharrell Williams' }));
    expect(commands.playbackReplace).not.toHaveBeenCalled();

    const page = await screen.findByTestId('music-artist-page');
    expect(within(page).getByRole('heading', { name: 'Pharrell Williams' })).toBeInTheDocument();
    expect(commands.musicArtist).toHaveBeenCalledWith('Pharrell Williams');
    const popular = await within(page).findByRole('region', { name: 'Popular' });
    expect(popular).toHaveTextContent('Happy');
    await user.click(within(page).getByRole('button', { name: 'Play all' }));
    expect(commands.playbackReplace).toHaveBeenCalledWith(
      expect.arrayContaining([expect.objectContaining({ title: 'Happy' })]),
      0,
    );
  });

  it("tells the artist's story from open databases", async () => {
    const user = userEvent.setup();
    commands.musicArtistInfo.mockResolvedValue(
      ok({
        name: 'Daft Punk',
        kind: 'Group',
        bio: 'Daft Punk were a French electronic music duo.',
        imageUrl: null,
        url: 'https://en.wikipedia.org/wiki/Daft_Punk',
        origin: 'Paris',
        active: '1993–2021',
        members: ['Thomas Bangalter', 'Guy-Manuel de Homem-Christo'],
        providerName: 'MusicBrainz',
      }),
    );
    await AppShellWrapper.mount('/artist?name=Daft+Punk');

    const about = await screen.findByTestId('artist-about');
    expect(about).toHaveTextContent('Group · Paris · 1993–2021');
    expect(about).toHaveTextContent('Daft Punk were a French electronic music duo.');
    expect(about).toHaveTextContent('Members: Thomas Bangalter, Guy-Manuel de Homem-Christo');
    expect(commands.musicArtistInfo).toHaveBeenCalledWith('Daft Punk');
    await user.click(within(about).getByRole('button', { name: 'Read more' }));
    expect(commands.musicOpenInfoPage).toHaveBeenCalledWith(
      'https://en.wikipedia.org/wiki/Daft_Punk',
    );
  });
});
