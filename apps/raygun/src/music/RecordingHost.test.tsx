import type { SongDownload } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { RecordingHost } from './RecordingHost';
import { recordingsOf } from './songDownloadState';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const keep = () => {};

const download = (
  id: string,
  title: string,
  state: SongDownload['state'],
  extra: Partial<SongDownload> = {},
): SongDownload => ({
  id,
  trackKey: `a|${title.toLowerCase()}`,
  title,
  artist: 'Band',
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  state,
  kind: 'recording',
  provider: null,
  format: null,
  doneBytes: 0,
  totalBytes: null,
  fraction: null,
  bytesPerSecond: null,
  etaMs: null,
  error: null,
  errorDetail: null,
  replacing: false,
  problem: null,
  queuedAt: 1,
  finishedAt: null,
  ...extra,
});

describe('recordingsOf', () => {
  it('puts the song recording now first, then the queue in its order', () => {
    const list = [
      download('late', 'Late', 'queued', { queuedAt: 9 }),
      download('file', 'File', 'queued', { kind: 'file' }),
      download('done', 'Done', 'done'),
      download('now', 'Now', 'downloading', { queuedAt: 20 }),
      download('early', 'Early', 'queued', { queuedAt: 2 }),
      download('stopped', 'Stopped', 'failed'),
    ];
    expect(recordingsOf(list).map((item) => item.id)).toEqual(['now', 'early', 'late']);
  });
});

describe('RecordingHost', () => {
  it('shows every song to record in one card, the one recording now with how far it is', async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(
      ok([
        download('1', 'Ballad', 'downloading', { fraction: 0.42 }),
        download('2', 'Chorus', 'queued', { queuedAt: 2 }),
        download('3', 'Anthem', 'queued', { queuedAt: 3 }),
        download('4', 'Elsewhere', 'queued', { kind: 'file' }),
      ]),
    );
    renderWithProviders(<RecordingHost onMinimize={keep} />);

    const card = await screen.findByRole('region', { name: 'Recording 3 songs' });
    expect(card).toHaveTextContent('Recording Ballad · 42%');
    expect(card).toHaveTextContent('silently in the background: keep listening');
    expect(within(card).getByRole('progressbar', { name: 'Recording progress' })).toHaveAttribute(
      'aria-valuenow',
      '42',
    );

    await user.click(within(card).getByRole('button', { name: 'Show the songs to record' }));
    const queue = within(card).getByTestId('recording-queue');
    expect(
      within(queue)
        .getAllByRole('listitem')
        .map((row) => row.textContent),
    ).toEqual(['Ballad – Band42%', 'Chorus – BandWaiting', 'Anthem – BandWaiting']);

    // One song, or all of them.
    await user.click(within(queue).getByRole('button', { name: 'Stop recording Chorus' }));
    expect(commands.songDownloadCancelRecording).toHaveBeenCalledWith('2');
    await user.click(within(card).getByRole('button', { name: 'Stop recording' }));
    expect(commands.songDownloadCancelRecording).toHaveBeenCalledWith(null);
  });

  it('says why the queue waits when nothing records yet', async () => {
    commands.songDownloads.mockResolvedValue(
      ok([
        download('1', 'Ballad', 'queued', {
          error: 'Waits until Raygun is open: it’s recorded in Raygun’s window.',
        }),
      ]),
    );
    renderWithProviders(<RecordingHost onMinimize={keep} />);

    const card = await screen.findByRole('region', { name: 'Recording 1 song' });
    expect(card).toHaveTextContent('Waits until Raygun is open');
    expect(within(card).queryByRole('progressbar')).not.toBeInTheDocument();
  });

  it('is minimized from its own button (the dock keeps it as a round button)', async () => {
    const user = userEvent.setup();
    const minimize = vi.fn();
    commands.songDownloads.mockResolvedValue(ok([download('1', 'Ballad', 'downloading')]));
    renderWithProviders(<RecordingHost onMinimize={minimize} />);

    const card = await screen.findByRole('region', { name: 'Recording 1 song' });
    expect(within(card).queryByRole('button', { name: 'Hide' })).not.toBeInTheDocument();
    await user.click(within(card).getByRole('button', { name: 'Minimize' }));
    expect(minimize).toHaveBeenCalledTimes(1);
  });

  it('draws nothing while no song is to be recorded', async () => {
    commands.songDownloads.mockResolvedValue(
      ok([download('1', 'Ballad', 'queued', { kind: 'file' })]),
    );
    renderWithProviders(<RecordingHost onMinimize={keep} />);
    await vi.waitFor(() => expect(commands.songDownloads).toHaveBeenCalled());
    expect(screen.queryByRole('region')).not.toBeInTheDocument();
  });
});
