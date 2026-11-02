import type { SavedTrack, SongDownload } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

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

const download = (
  id: string,
  title: string,
  state: SongDownload['state'],
  extra: Partial<SongDownload> = {},
): SongDownload => ({
  id,
  trackKey: `a|${title.toLowerCase()}`,
  title,
  artist: 'A',
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  state,
  kind: null,
  provider: null,
  format: null,
  doneBytes: 0,
  totalBytes: null,
  error: null,
  canRecord: true,
  ...extra,
});

const setWide = () => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: true,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
};

describe('Song downloads', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.favoritesList.mockResolvedValue(
      ok([saved('Get Lucky', 'Daft Punk'), saved('Thunderstruck', 'AC/DC')]),
    );
  });

  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('downloads from a song menu and the selection, and marks downloaded songs', async () => {
    setWide();
    const user = userEvent.setup();
    commands.songDownloadedKeys.mockResolvedValue(['ac/dc|thunderstruck']);
    await AppShellWrapper.mount('/library?view=favorites');
    const rows = await screen.findAllByTestId('track-table-row');
    expect(
      await within(rows[1] as HTMLElement).findByRole('img', { name: 'Downloaded' }),
    ).toBeInTheDocument();
    expect(within(rows[0] as HTMLElement).queryByRole('img', { name: 'Downloaded' })).toBeNull();

    fireEvent.contextMenu(rows[0] as HTMLElement);
    await user.click(await screen.findByRole('menuitem', { name: 'Download' }));
    expect(commands.songsDownload).toHaveBeenCalledWith([
      expect.objectContaining({ title: 'Get Lucky', artist: 'Daft Punk' }),
    ]);

    // Several at once from the selection bar.
    await user.click(within(rows[0] as HTMLElement).getByRole('button', { name: 'Get Lucky' }));
    fireEvent.click(within(rows[1] as HTMLElement).getAllByRole('button')[1] as HTMLElement, {
      ctrlKey: true,
    });
    const bar = await screen.findByRole('toolbar');
    await user.click(within(bar).getByRole('button', { name: 'Download' }));
    expect(commands.songsDownload).toHaveBeenLastCalledWith(expect.any(Array));
  });

  it('lists downloads with progress, records YouTube-only songs and saves files', async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(
      ok([
        download('1', 'Queued song', 'queued'),
        download('2', 'Coming in', 'downloading', { doneBytes: 450, totalBytes: 1000 }),
        download('3', 'Only on YouTube', 'needsRecording', {
          error: 'Only YouTube has this song',
        }),
        download('4', 'Broken', 'failed', { error: 'The server answered 404' }),
        download('5', 'Saved song', 'done', { format: 'mp3', provider: 'Internet Archive' }),
      ]),
    );
    commands.songsSave.mockResolvedValue(
      ok({ saved: ['Music/Raygun/A/A - Saved song.flac'], failed: [] }),
    );
    await AppShellWrapper.mount('/downloads?tab=music');
    const list = await screen.findByTestId('song-downloads');
    const row = (title: string) => within(list).getByRole('listitem', { name: title });

    expect(await within(list).findByText('Queued')).toBeInTheDocument();
    expect(row('Coming in')).toHaveTextContent('45%');
    expect(row('Saved song')).toHaveTextContent('On this device · MP3 · Internet Archive');
    expect(row('Broken')).toHaveTextContent('The server answered 404');

    await user.click(within(row('Only on YouTube')).getByRole('button', { name: 'Record' }));
    expect(commands.songDownloadRecord).toHaveBeenCalledWith('3');
    await user.click(within(row('Broken')).getByRole('button', { name: 'Try Broken again' }));
    expect(commands.songDownloadRetry).toHaveBeenCalledWith('4');
    await user.click(
      within(row('Queued song')).getByRole('button', { name: 'Remove Queued song' }),
    );
    expect(commands.songDownloadRemove).toHaveBeenCalledWith('1');

    await user.click(
      within(row('Saved song')).getByRole('button', { name: 'Save Saved song as a file' }),
    );
    const sheet = await screen.findByRole('dialog', { name: 'Save the song as a file' });
    await user.click(within(sheet).getByRole('radio', { name: /^FLAC/ }));
    await user.click(within(sheet).getByRole('button', { name: 'Save' }));
    expect(commands.songsSave).toHaveBeenCalledWith(['5'], 'flac', 'music');
    expect(
      await within(sheet).findByText('Saved to Music/Raygun/A/A - Saved song.flac'),
    ).toBeInTheDocument();
  });

  it('chooses how songs are written in Settings → Downloads', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/downloads');
    await user.click(await screen.findByRole('button', { name: /Songs saved as/ }));
    await user.click(await screen.findByRole('radio', { name: 'FLAC' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'save',
      value: expect.objectContaining({ songFormat: 'flac' }),
    });
  });
});
