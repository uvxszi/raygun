import type { SavedTrack, SongDownload } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const download = (
  state: SongDownload['state'],
  extra: Partial<SongDownload> = {},
): SongDownload => ({
  id: 'd1',
  trackKey: 'nina simone|sinnerman',
  title: 'Sinnerman',
  artist: 'Nina Simone',
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  state,
  kind: null,
  provider: null,
  format: null,
  doneBytes: 0,
  totalBytes: null,
  fraction: null,
  bytesPerSecond: null,
  etaMs: null,
  error: null,
  errorDetail: null,
  canRecord: false,
  queuedAt: 1,
  finishedAt: null,
  ...extra,
});

const saved = (title: string): SavedTrack => ({
  trackKey: `nina simone|${title.toLowerCase()}`,
  title,
  artist: 'Nina Simone',
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  source: null,
  local: false,
  addedAt: 1,
  playCount: 0,
});

/** The full player's download button, for the playing song. */
const openPlayer = async () => {
  await AppShellWrapper.mount('/');
  AppShellWrapper.playing([makeItem('Sinnerman', 'Nina Simone')], 0);
  await AppShellWrapper.miniPlayer.open();
  return AppShellWrapper.playerSheet.element;
};

describe('Downloads beside the Plus (M46.18)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('downloads the playing song, shows it on its way, then asks before removing it', async () => {
    const user = userEvent.setup();
    const sheet = await openPlayer();

    const button = await within(sheet).findByRole('button', { name: 'Download' });
    await vi.waitFor(() => expect(button).toBeEnabled());
    await user.click(button);
    expect(commands.songsDownload).toHaveBeenCalledWith([
      expect.objectContaining({ title: 'Sinnerman', artist: 'Nina Simone' }),
    ]);

    // Rust queues it and it comes in.
    emitEvent('songDownloadProgressEvent', download('queued'));
    expect(await within(sheet).findByRole('button', { name: 'Downloading' })).toBeInTheDocument();
    emitEvent(
      'songDownloadProgressEvent',
      download('downloading', { provider: 'Audius', fraction: 0.5, doneBytes: 5, totalBytes: 10 }),
    );
    await user.click(within(sheet).getByRole('button', { name: 'Downloading' }));
    const progress = await screen.findByTestId('download-confirm');
    expect(progress).toHaveTextContent('Cancel this download?');
    expect(progress).toHaveTextContent('Audius · 50%');
    await user.click(within(progress).getByRole('button', { name: 'Keep' }));

    // Once it's downloaded the button fills in, and removing it takes a confirmation.
    emitEvent('songDownloadProgressEvent', download('done', { format: 'mp3', fraction: 1 }));
    const done = await within(sheet).findByRole('button', { name: 'Downloaded' });
    await user.click(done);
    const confirm = await screen.findByTestId('download-confirm');
    expect(confirm).toHaveTextContent('Remove from downloads?');
    await user.click(within(confirm).getByRole('button', { name: 'Keep' }));
    expect(commands.songDownloadRemove).not.toHaveBeenCalled();
    await user.click(done);
    await user.click(
      within(await screen.findByTestId('download-confirm')).getByRole('button', { name: 'Remove' }),
    );
    expect(commands.songDownloadRemove).toHaveBeenCalledWith('d1');
    // Gone at once, and nothing left open.
    expect(await within(sheet).findByRole('button', { name: 'Download' })).toBeInTheDocument();
    expect(screen.queryByTestId('download-confirm')).not.toBeInTheDocument();
  });

  it('says why a download failed, and shows it on its way again at once after Try again', async () => {
    const user = userEvent.setup();
    commands.songDownloadRetry.mockReturnValue(new Promise(() => undefined));
    commands.songDownloads.mockResolvedValue(
      ok([
        download('failed', {
          error: 'No source has this song. Choose another source.',
          errorDetail: 'Search: No streaming source has “Sinnerman”',
        }),
      ]),
    );
    const sheet = await openPlayer();
    await user.click(await within(sheet).findByRole('button', { name: 'Download didn’t work' }));
    const confirm = await screen.findByTestId('download-confirm');
    expect(confirm).toHaveTextContent('No source has this song. Choose another source.');
    await user.click(within(confirm).getByRole('button', { name: 'More info' }));
    expect(confirm).toHaveTextContent('Search: No streaming source has “Sinnerman”');
    await user.click(within(confirm).getByRole('button', { name: 'Try again' }));
    expect(commands.songDownloadRetry).toHaveBeenCalledWith('d1');
    // Before Rust answers: on its way, not the warning.
    expect(within(sheet).getByRole('button', { name: 'Downloading' })).toBeInTheDocument();
    expect(within(sheet).queryByRole('button', { name: 'Download didn’t work' })).toBeNull();
    expect(screen.queryByTestId('download-confirm')).not.toBeInTheDocument();
  });

  it('lists downloaded songs in Library → Audio → Downloaded and plays them', async () => {
    const user = userEvent.setup();
    commands.downloadedSongs.mockResolvedValue(ok([saved('Sinnerman'), saved('Feeling Good')]));
    await AppShellWrapper.mount('/library?view=all');
    const overview = await screen.findByTestId('music-overview');
    await user.click(await within(overview).findByRole('link', { name: /Downloaded/ }));

    const view = await screen.findByTestId('downloaded-view');
    const rows = await within(view).findAllByTestId('track-table-row');
    expect(rows).toHaveLength(2);
    await user.click(within(view).getByRole('button', { name: 'Shuffle' }));
    expect(commands.collectionPlay).toHaveBeenCalledWith({
      collection: { kind: 'downloaded' },
      startIndex: null,
      shuffle: true,
    });
    expect(within(view).getByRole('link', { name: 'Manage downloads' })).toBeInTheDocument();
  });

  it('says how to download when nothing is downloaded', async () => {
    await AppShellWrapper.mount('/library?view=downloaded');
    expect(await screen.findByText('No downloads yet')).toBeInTheDocument();
  });

  it('saves as MP3 and picks a folder from the save sheet', async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(ok([download('done', { format: 'm4a' })]));
    commands.songsSave.mockResolvedValue(
      ok({ saved: ['Song folder/A - Sinnerman.mp3'], failed: [] }),
    );
    commands.songsPickFolder.mockResolvedValue(
      ok({
        ...DEFAULT_SETTINGS,
        save: {
          ...DEFAULT_SETTINGS.save,
          folderUri: 'content://tree/music',
          folderName: 'Song folder',
        },
      }),
    );
    await AppShellWrapper.mount('/downloads?tab=music');
    await user.click(await screen.findByRole('button', { name: 'Save Sinnerman as a file' }));
    const sheet = await screen.findByRole('dialog', { name: 'Save the song as a file' });
    await user.click(within(sheet).getByRole('radio', { name: /^MP3/ }));
    await user.click(within(sheet).getByRole('button', { name: 'Choose a folder' }));
    expect(commands.songsPickFolder).toHaveBeenCalled();
    const folder = await within(sheet).findByRole('radio', { name: 'Song folder' });
    expect(folder).toBeChecked();
    await user.click(within(sheet).getByRole('button', { name: 'Save' }));
    expect(commands.songsSave).toHaveBeenCalledWith(['d1'], 'mp3', 'folder');
  });

  it('sets the audio quality for streaming, mobile data and downloads', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/player');
    const mobile = await screen.findByRole('button', { name: /Mobile data/ });
    expect(mobile).toHaveTextContent('Normal');
    await user.click(mobile);
    await user.click(await screen.findByRole('radio', { name: /^Low/ }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'audioQuality',
      value: { streaming: 'high', cellular: 'low', download: 'veryHigh' },
    });
  });
});
