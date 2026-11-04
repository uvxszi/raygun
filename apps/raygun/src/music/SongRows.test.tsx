import type { OnlineTrack, SavedTrack } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const PROVIDER = {
  id: 'dev.raygun.deezer/catalogue',
  extensionId: 'dev.raygun.deezer',
  kind: 'music.metadata',
  name: 'Deezer',
};

const song = (title: string, artist: string): OnlineTrack => ({
  externalId: null,
  title,
  artist,
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
  explicit: false,
});

const saved = (title: string, artist: string): SavedTrack => ({
  trackKey: `${artist.toLowerCase()}|${title.toLowerCase()}`,
  title,
  artist,
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  source: null,
  local: false,
  addedAt: 1,
  playCount: 0,
});

/** A phone (a finger) or a computer (a mouse), as `(pointer: coarse)` tells them apart. */
const pointer = (coarse: boolean) => {
  window.matchMedia = ((query: string) => ({
    matches: coarse && query === '(pointer: coarse)',
    media: query,
    onchange: null,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    addListener: () => undefined,
    removeListener: () => undefined,
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
};

const searchFor = async () => {
  commands.onlineSearch.mockResolvedValue(
    ok({
      library: [],
      top: [],
      providers: [
        {
          provider: PROVIDER,
          tracks: [song('Get Lucky', 'Daft Punk, Pharrell Williams')],
          error: null,
        },
      ],
    }),
  );
  await AppShellWrapper.mount('/browse?tab=search&type=music&view=songs&q=get+lucky');
  return screen.findByTestId('online-track-row');
};

describe('Song rows', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });
  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('keep the artists as links with a mouse, apart from the title, opening without playing', async () => {
    const user = userEvent.setup();
    pointer(false);
    const row = await searchFor();
    const title = within(row).getByText('Get Lucky');
    const artist = within(row).getByRole('link', { name: 'Pharrell Williams' });
    // Title and artists are lines of their own, with space between them.
    expect(title.closest('.flex-col')).toHaveClass('gap-1.5');
    expect(artist.textContent).toBe('Pharrell Williams');

    await user.click(artist);
    expect(await screen.findByTestId('music-artist-page')).toBeInTheDocument();
    expect(commands.playbackReplace).not.toHaveBeenCalled();
  });

  it('make the artists plain text on a touch screen, so a tap anywhere plays the song', async () => {
    const user = userEvent.setup();
    pointer(true);
    const row = await searchFor();
    expect(within(row).queryByRole('link')).not.toBeInTheDocument();
    await user.click(within(row).getByText('Daft Punk, Pharrell Williams'));
    expect(commands.playbackReplace).toHaveBeenCalledWith(
      [expect.objectContaining({ title: 'Get Lucky' })],
      0,
    );
  });

  it('play a song table row from a tap anywhere on a touch screen', async () => {
    const user = userEvent.setup();
    pointer(true);
    commands.favoritesList.mockResolvedValue(
      ok([saved('Sinnerman', 'Nina Simone'), saved('At Last', 'Etta James')]),
    );
    await AppShellWrapper.mount('/library?view=favorites');
    const rows = await screen.findAllByTestId('track-table-row');
    const second = rows[1] as HTMLElement;
    expect(within(second).queryByRole('link')).not.toBeInTheDocument();
    await user.click(within(second).getByText('Etta James'));
    expect(commands.collectionPlay).toHaveBeenCalledWith(
      expect.objectContaining({ collection: { kind: 'favorites' } }),
    );
    expect(second).toHaveAttribute('aria-selected', 'false');
  });
});
