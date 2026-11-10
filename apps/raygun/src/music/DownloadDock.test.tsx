import type { SongDownload } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { dockSummary, useDownloadDockStore } from './downloadDockStore';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

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

const ring = (button: HTMLElement) =>
  within(button).getByTestId('download-dock-ring').getAttribute('stroke-dashoffset');

describe('dockSummary', () => {
  it('counts the songs left and how far the batch is, finished ones included', () => {
    const since = 1_000;
    const list = [
      download('rec', 'Rec', 'downloading', { fraction: 0.5 }),
      download('wait', 'Wait', 'queued'),
      download('file', 'File', 'downloading', { kind: 'file', fraction: 0.5 }),
      // Finished during the batch: counts whole, and keeps the ring from going back.
      download('done', 'Done', 'done', { kind: 'file', finishedAt: 2_000 }),
      download('failed', 'Failed', 'failed', { kind: 'file', finishedAt: 3_000 }),
      // Finished before it started: not part of it.
      download('old', 'Old', 'done', { kind: 'file', finishedAt: 500 }),
    ];
    expect(dockSummary(list, since)).toEqual({ left: 3, recordings: 2, progress: 0.6 });
    expect(dockSummary(list, null)).toEqual({ left: 3, recordings: 2, progress: 0.33 });
    expect(dockSummary([download('old', 'Old', 'done', { finishedAt: 500 })], since)).toEqual({
      left: 0,
      recordings: 0,
      progress: 0,
    });
  });
});

describe('The download dock', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    sessionStorage.clear();
    useDownloadDockStore.setState({ minimized: false, since: null });
  });

  it('lays the recording bar out above the player bar, minimized into a round button for the session', async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(
      ok([
        download('1', 'Ballad', 'downloading', { fraction: 0.5 }),
        download('2', 'Chorus', 'queued', { queuedAt: 2 }),
        download('3', 'Elsewhere', 'downloading', { kind: 'file', fraction: 0.25 }),
      ]),
    );
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro', 'Raygun')], 0);
    await AppShellWrapper.miniPlayer.title;

    // In the layout right above the player bar: it covers neither the bar nor the page.
    const card = await screen.findByRole('region', { name: 'Recording 2 songs' });
    const dock = screen.getByTestId('download-dock');
    expect(dock).toContainElement(card);
    expect(dock.nextElementSibling).toBe(AppShellWrapper.miniPlayer.element);
    expect(screen.queryByRole('button', { name: /^Show the recording/ })).not.toBeInTheDocument();

    // Minimized: a round button on the right counting every song on its way, downloads too.
    await user.click(within(card).getByRole('button', { name: 'Minimize' }));
    const button = await screen.findByRole('button', {
      name: 'Show the recording: 3 songs left, 25%',
    });
    expect(button).toHaveTextContent('3');
    expect(ring(button)).toBe('75');
    expect(screen.queryByRole('region', { name: /^Recording/ })).not.toBeInTheDocument();
    expect(screen.getByTestId('download-dock').nextElementSibling).toBe(
      AppShellWrapper.miniPlayer.element,
    );
    expect(sessionStorage.getItem('raygun.downloadDock.minimized')).toBe('1');

    // Another song to record keeps it minimized; a song done counts toward the ring.
    emitEvent('songDownloadProgressEvent', download('4', 'Anthem', 'queued', { queuedAt: 5 }));
    emitEvent(
      'songDownloadProgressEvent',
      download('3', 'Elsewhere', 'done', { kind: 'file', finishedAt: Date.now() + 1_000 }),
    );
    const moved = await screen.findByRole('button', {
      name: 'Show the recording: 3 songs left, 38%',
    });
    expect(moved).toHaveTextContent('3');

    // A tap brings the bar back.
    await user.click(moved);
    expect(await screen.findByRole('region', { name: 'Recording 3 songs' })).toBeInTheDocument();
    expect(sessionStorage.getItem('raygun.downloadDock.minimized')).toBeNull();
  });

  it('shows song downloads in the round button, which opens Downloads → Music', async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(
      ok([
        download('1', 'Ballad', 'downloading', { kind: 'file', fraction: 0.5 }),
        download('2', 'Chorus', 'queued', { kind: 'file', queuedAt: 2 }),
      ]),
    );
    await AppShellWrapper.mount();

    // Nothing plays: it sits above the tabs.
    const button = await screen.findByRole('button', { name: 'Song downloads: 2 left, 25%' });
    expect(screen.getByTestId('download-dock').nextElementSibling).toBe(
      screen.getByTestId('bottom-tabs'),
    );
    await user.click(button);
    expect(await screen.findByTestId('downloads-page')).toBeInTheDocument();
    // Still there on the way to them: the songs are still downloading.
    expect(screen.getByRole('button', { name: 'Song downloads: 2 left, 25%' })).toBeInTheDocument();
  });

  it('draws nothing while no song is on its way', async () => {
    commands.songDownloads.mockResolvedValue(
      ok([download('1', 'Ballad', 'done', { kind: 'file', finishedAt: 5 })]),
    );
    await AppShellWrapper.mount();
    await waitFor(() => expect(commands.songDownloads).toHaveBeenCalled());
    expect(screen.queryByTestId('download-dock')).not.toBeInTheDocument();
  });
});
