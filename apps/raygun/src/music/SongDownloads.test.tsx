import type { OnlineTrack, SavedTrack, SongDownload } from '@raygun/bindings';
import { act, fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

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
  fraction: null,
  bytesPerSecond: null,
  etaMs: null,
  error: null,
  errorDetail: null,
  canRecord: true,
  queuedAt: 1,
  finishedAt: null,
  ...extra,
});

const BROKEN = download('4', 'Broken', 'failed', {
  error: 'Audius had it, but the download didn’t work. Choose another source.',
  errorDetail: 'Audius: The server answered 404 Not Found',
  finishedAt: 50,
});

const setWide = () => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: true,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
};

const list = () => screen.findByTestId('song-downloads');
const row = (title: string) =>
  within(screen.getByTestId('song-downloads')).getByRole('listitem', { name: title });

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
    commands.songDownloads.mockResolvedValue(
      ok([
        download('t', 'Thunderstruck', 'done', {
          trackKey: 'ac/dc|thunderstruck',
          artist: 'AC/DC',
        }),
      ]),
    );
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

    // Several at once from the selection bar; a song saved already is left out (M46.46).
    await user.click(screen.getByRole('button', { name: 'Select all' }));
    const bar = await screen.findByRole('toolbar');
    await user.click(within(bar).getByRole('button', { name: 'Download 1 not saved yet' }));
    expect(commands.songsDownload).toHaveBeenLastCalledWith([
      expect.objectContaining({ title: 'Get Lucky' }),
    ]);

    // Rust queues it: the row says it's waiting, then downloading, and the bar leaves it out.
    emitEvent(
      'songDownloadProgressEvent',
      download('g', 'Get Lucky', 'queued', { trackKey: 'daft punk|get lucky' }),
    );
    expect(
      await within(rows[0] as HTMLElement).findByRole('img', { name: 'Waiting to download' }),
    ).toBeInTheDocument();
    expect(within(bar).getByRole('button', { name: 'All downloaded' })).toBeDisabled();
    emitEvent(
      'songDownloadProgressEvent',
      download('g', 'Get Lucky', 'downloading', { trackKey: 'daft punk|get lucky' }),
    );
    expect(
      await within(rows[0] as HTMLElement).findByRole('img', { name: 'Downloading' }),
    ).toBeInTheDocument();
    // Select all again clears the selection and closes its bar.
    await user.click(screen.getByRole('button', { name: 'Select all' }));
    expect(screen.queryByRole('toolbar')).not.toBeInTheDocument();
  });

  it('lists downloads with progress, records songs that need it and saves files', async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(
      ok([
        download('1', 'Queued song', 'queued'),
        download('2', 'Coming in', 'downloading', {
          provider: 'Internet Archive',
          doneBytes: 450,
          totalBytes: 1000,
          fraction: 0.45,
        }),
        download('3', 'Only on YouTube', 'needsRecording', {
          error: 'Only YouTube has this song. Record it as it plays, or choose another source.',
        }),
        BROKEN,
        download('5', 'Saved song', 'done', { format: 'mp3', provider: 'Internet Archive' }),
      ]),
    );
    commands.songsSave.mockResolvedValue(
      ok({ saved: ['Music/Raygun/A/A - Saved song.flac'], failed: [] }),
    );
    await AppShellWrapper.mount('/downloads?tab=music');
    await list();

    expect(await within(row('Queued song')).findByText('Queued')).toBeInTheDocument();
    expect(row('Coming in')).toHaveTextContent('Internet Archive · 45%');
    expect(within(row('Coming in')).getByRole('progressbar')).toHaveAttribute(
      'aria-valuenow',
      '45',
    );
    expect(row('Saved song')).toHaveTextContent('On this device · MP3 · Internet Archive');
    // What went wrong, with the source's own error under More info.
    expect(row('Broken')).toHaveTextContent('Audius had it, but the download didn’t work');
    expect(row('Broken')).not.toHaveTextContent('404');
    await user.click(within(row('Broken')).getByRole('button', { name: 'More info' }));
    expect(row('Broken')).toHaveTextContent('Audius: The server answered 404 Not Found');
    expect(row('Only on YouTube')).toHaveTextContent('Can be recorded as it plays');

    await user.click(within(row('Only on YouTube')).getByRole('button', { name: 'Record' }));
    expect(commands.songDownloadRecord).toHaveBeenCalledWith('3');
    await user.click(
      within(row('Queued song')).getByRole('button', { name: 'Cancel Queued song' }),
    );
    expect(commands.songDownloadRemove).toHaveBeenCalledWith('1');
    expect(screen.queryByRole('listitem', { name: 'Queued song' })).not.toBeInTheDocument();

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

  it('shows a song tried again as queued at once, then its progress', async () => {
    const user = userEvent.setup();
    let answer: () => void = () => undefined;
    commands.songDownloadRetry.mockReturnValue(
      new Promise((resolve) => {
        answer = () => resolve(ok(null));
      }),
    );
    commands.songDownloads.mockResolvedValue(ok([BROKEN]));
    await AppShellWrapper.mount('/downloads?tab=music');
    await list();
    expect(row('Broken')).toHaveTextContent("Couldn't download");

    await user.click(within(row('Broken')).getByRole('button', { name: 'Try Broken again' }));
    // Before Rust answers: queued, the error gone, nothing left to try again.
    await vi.waitFor(() => expect(row('Broken')).toHaveTextContent('Queued'));
    expect(commands.songDownloadRetry).toHaveBeenCalledWith('4');
    expect(row('Broken')).not.toHaveTextContent('Audius had it');
    expect(within(row('Broken')).queryByRole('button', { name: 'Try Broken again' })).toBeNull();
    expect(
      within(screen.getByRole('list', { name: 'Downloading' })).getByRole('listitem', {
        name: 'Broken',
      }),
    ).toBeInTheDocument();

    // Rust starts it and reports how far it is.
    emitEvent(
      'songDownloadProgressEvent',
      download('4', 'Broken', 'downloading', {
        provider: 'Audius',
        doneBytes: 450_000,
        totalBytes: 1_000_000,
        fraction: 0.45,
        etaMs: 12_000,
      }),
    );
    await vi.waitFor(() => expect(row('Broken')).toHaveTextContent('Audius · 45%'));
    expect(row('Broken')).toHaveTextContent('0:12 left');
    expect(within(row('Broken')).getByRole('progressbar')).toHaveAttribute('aria-valuenow', '45');
    await act(async () => answer());
    expect(row('Broken')).toHaveTextContent('Audius · 45%');
  });

  it('never brings back a failure that an answer from before the retry still had', async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(ok([BROKEN]));
    await AppShellWrapper.mount('/downloads?tab=music');
    await list();

    // Rust said something changed; its answer is on the way when Try again is pressed.
    let answer: (value: unknown) => void = () => undefined;
    commands.songDownload.mockReturnValue(
      new Promise((resolve) => {
        answer = resolve;
      }),
    );
    emitEvent('songDownloadsEvent', '4');
    await vi.waitFor(() => expect(commands.songDownload).toHaveBeenCalledWith('4'));
    await user.click(within(row('Broken')).getByRole('button', { name: 'Try Broken again' }));
    await vi.waitFor(() => expect(row('Broken')).toHaveTextContent('Queued'));
    await act(async () => answer(ok(BROKEN)));
    await act(() => new Promise((resolve) => setTimeout(resolve, 20)));
    expect(row('Broken')).toHaveTextContent('Queued');
    expect(row('Broken')).not.toHaveTextContent('Audius had it');

    // A removal (no download follows the change) is asked about and taken out.
    commands.songDownload.mockResolvedValue(ok(null));
    emitEvent('songDownloadsEvent', '4');
    await vi.waitFor(() =>
      expect(screen.queryByRole('listitem', { name: 'Broken' })).not.toBeInTheDocument(),
    );
  });

  it('downloads from a picked source, shown as queued from it at once', async () => {
    const user = userEvent.setup();
    const track = (source: OnlineTrack['source'], externalId: string): OnlineTrack => ({
      externalId,
      title: 'Broken',
      artist: 'A',
      album: null,
      durationMs: 200_000,
      artworkUrl: null,
      source,
      explicit: false,
    });
    const audius = track({ kind: 'url', url: 'https://audius.co/broken.mp3', headers: {} }, 'a1');
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: {
              id: 'audius/s',
              extensionId: 'audius',
              kind: 'music.search',
              name: 'Audius',
            },
            tracks: [audius],
            error: null,
          },
          {
            provider: { id: 'yt/s', extensionId: 'youtube', kind: 'music.search', name: 'YouTube' },
            tracks: [track({ kind: 'youTube', videoId: 'abc' }, 'y1')],
            error: null,
          },
        ],
      }),
    );
    let answer: () => void = () => undefined;
    commands.songDownloadUseSource.mockReturnValue(
      new Promise((resolve) => {
        answer = () => resolve(ok(null));
      }),
    );
    commands.songDownloads.mockResolvedValue(ok([BROKEN]));
    await AppShellWrapper.mount('/downloads?tab=music');
    await list();

    await user.click(
      within(row('Broken')).getByRole('button', { name: 'Choose a source for Broken' }),
    );
    const sheet = await screen.findByTestId('source-picker');
    // Why it's asked, then only what this phone can save (not YouTube's copy).
    expect(sheet).toHaveTextContent('Audius had it, but the download didn’t work');
    const offers = await within(sheet).findAllByRole('button', { name: 'Download from this' });
    expect(offers).toHaveLength(1);

    // A preview joins the queue instead of replacing it.
    await user.click(within(sheet).getByRole('button', { name: 'Preview: Broken' }));
    expect(commands.playbackEnqueue).toHaveBeenCalledWith(
      [expect.objectContaining({ title: 'Broken' })],
      true,
    );
    expect(commands.playbackReplace).not.toHaveBeenCalled();

    await user.click(offers[0] as HTMLElement);
    expect(commands.songDownloadUseSource).toHaveBeenCalledWith('4', audius, 'Audius');
    await vi.waitFor(() => expect(row('Broken')).toHaveTextContent('Queued · Audius'));
    expect(row('Broken')).not.toHaveTextContent('download didn’t work');
    await act(async () => answer());
    await vi.waitFor(() => expect(screen.queryByTestId('source-picker')).not.toBeInTheDocument());
  });

  it('pauses, reorders, cancels and tries the failed ones again', async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(
      ok([
        download('now', 'Now', 'downloading', { provider: 'Audius', fraction: 0.2 }),
        download('qa', 'Queued A', 'queued', { queuedAt: 10 }),
        download('qb', 'Queued B', 'queued', { queuedAt: 20 }),
        BROKEN,
        download('3', 'Only here', 'needsRecording', { error: 'Only YouTube has this song.' }),
      ]),
    );
    await AppShellWrapper.mount('/downloads?tab=music');
    await list();
    const queue = () =>
      within(screen.getByRole('list', { name: 'Downloading' }))
        .getAllByRole('listitem')
        .map((item) => item.getAttribute('aria-label'));
    // Downloading now, then the queue in the order it runs.
    expect(queue()).toEqual(['Now', 'Queued A', 'Queued B']);

    await user.click(
      within(row('Queued B')).getByRole('button', { name: 'Download Queued B next' }),
    );
    expect(commands.songDownloadsReorder).toHaveBeenCalledWith(['qb']);
    await vi.waitFor(() => expect(queue()).toEqual(['Now', 'Queued B', 'Queued A']));

    // Rust pauses (the running one waits in its place).
    commands.songDownloadsSetPaused.mockImplementation(async (paused: boolean) => {
      commands.songDownloadsPaused.mockResolvedValue(paused);
      return null;
    });
    await user.click(screen.getByRole('button', { name: 'Pause downloads' }));
    expect(commands.songDownloadsSetPaused).toHaveBeenCalledWith(true);
    await vi.waitFor(() => expect(row('Queued A')).toHaveTextContent('Paused'));
    expect(screen.getByRole('button', { name: 'Resume downloads' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Cancel 2 waiting' }));
    expect(commands.songDownloadsRemove).toHaveBeenCalledWith(['qb', 'qa']);
    await vi.waitFor(() => expect(queue()).toEqual(['Now']));

    await user.click(screen.getByRole('button', { name: 'Try 2 again' }));
    expect(commands.songDownloadsRetryFailed).toHaveBeenCalled();
    await vi.waitFor(() => expect(queue()).toEqual(['Now', 'Broken', 'Only here']));
    expect(screen.queryByRole('button', { name: /failed$/ })).not.toBeInTheDocument();
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
