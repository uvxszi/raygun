import type { SongDownload } from '@raygun/bindings';
import { act, screen } from '@testing-library/react';
import { Profiler } from 'react';
import type { FC } from 'react';

import { bindingsMock, emitEvent, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { DownloadSongButton } from './DownloadSongButton';
import { SongDownloadsHost } from './SongDownloadsHost';
import { SongMarks } from './SongMarks';
import { useDownloadedKeys, useDownloadingKeys } from './useSongDownloads';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const download = (
  title: string,
  state: SongDownload['state'],
  extra: Partial<SongDownload> = {},
): SongDownload => ({
  id: title,
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
  replacing: false,
  problem: null,
  queuedAt: 1,
  finishedAt: null,
  ...extra,
});

const track = (title: string) => ({
  title,
  artist: 'A',
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  source: null,
});

/** Lets queued updates (React Query tells its observers on a timer) reach the screen. */
const settle = () => act(() => new Promise((resolve) => setTimeout(resolve, 10)));

/** What the song lists read: which songs are downloaded or on their way. */
const Marks: FC = () => {
  const downloaded = useDownloadedKeys();
  const downloading = useDownloadingKeys();
  return (
    <span data-testid="keys">
      {downloaded.size} {downloading.size}
    </span>
  );
};

describe('Song download progress', () => {
  it('redraws only the song that moved, a few times, never the lists', async () => {
    seedBindings();
    commands.songDownloads.mockResolvedValue(
      ok([
        download('Moving', 'downloading'),
        download('Waiting', 'queued'),
        download('Here', 'done'),
      ]),
    );
    const commits = new Map<string, number>();
    const count = (id: string) => () => commits.set(id, (commits.get(id) ?? 0) + 1);
    const titles = Array.from({ length: 40 }, (_, index) => `Song ${index}`);
    renderWithProviders(
      <>
        <SongDownloadsHost />
        <Profiler id="marks" onRender={count('marks')}>
          {[...titles, 'Moving', 'Waiting', 'Here'].map((title) => (
            <SongMarks key={title} title={title} artist="A" />
          ))}
          <Marks />
        </Profiler>
        <Profiler id="other" onRender={count('other')}>
          <DownloadSongButton track={track('Here')} />
        </Profiler>
        <Profiler id="moving" onRender={count('moving')}>
          <DownloadSongButton track={track('Moving')} />
        </Profiler>
      </>,
    );
    expect(await screen.findByRole('button', { name: 'Downloaded' })).toBeInTheDocument();
    expect(await screen.findByTestId('keys')).toHaveTextContent('1 2');
    await settle();
    const before = new Map(commits);
    const since = (id: string) => (commits.get(id) ?? 0) - (before.get(id) ?? 0);

    // Reports come a few times a second; each is drawn before the next.
    for (let step = 1; step <= 10; step += 1) {
      emitEvent(
        'songDownloadProgressEvent',
        download('Moving', 'downloading', {
          provider: 'Audius',
          doneBytes: step * 100,
          totalBytes: 1000,
          fraction: step / 10,
        }),
      );
      await settle();
    }

    expect(since('marks')).toBe(0);
    expect(since('other')).toBe(0);
    expect(since('moving')).toBe(10);

    // Finishing changes the marks once.
    emitEvent('songDownloadProgressEvent', download('Moving', 'done', { fraction: 1 }));
    await settle();
    expect(screen.getByTestId('keys')).toHaveTextContent('2 1');
    expect(since('marks')).toBe(1);
  });
});
