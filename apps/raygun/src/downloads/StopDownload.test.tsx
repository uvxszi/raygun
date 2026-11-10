import type { MangaChapterInfo, VideoDownloadItem } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { DownloadButton } from '../manga/DownloadButton';
import { bindingsMock } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { VideoDownloadControl } from '../watch/VideoDownloadControl';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const chapter = (state: 'queued' | 'downloading'): MangaChapterInfo => ({
  id: 'c1',
  name: 'Chapter 1',
  number: 1,
  volume: null,
  scanlator: null,
  dateUpload: null,
  read: false,
  lastPage: null,
  totalPages: null,
  download: { state, pagesDone: 4, pagesTotal: 16, error: null },
});

const episode: VideoDownloadItem = {
  id: 'd1',
  key: 'source:s:e1',
  titleId: 't1',
  title: 'Big Buck Bunny',
  subtitle: null,
  poster: null,
  state: 'downloading',
  doneBytes: 1024,
  totalBytes: 4096,
  doneParts: 0,
  totalParts: null,
  error: null,
  sourceId: 's',
  remoteId: 'bbb',
  episodeId: 'e1',
  watched: false,
  at: 0,
};

describe('Stopping a download', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commands.mangaRemoveDownloads.mockResolvedValue(ok(null));
    commands.videoDownloadRemove.mockResolvedValue(ok(null));
  });

  it("asks before a chapter's progress ring stops it", async () => {
    const user = userEvent.setup();
    renderWithProviders(<DownloadButton chapter={chapter('downloading')} />);
    const ring = screen.getByRole('button', { name: 'Downloading Chapter 1, 25%. Cancel' });
    await user.click(ring);
    const ask = await screen.findByRole('dialog', { name: 'Stop this download?' });
    expect(ask).toHaveTextContent('What “Chapter 1” saved so far is deleted.');
    expect(ring).toHaveAttribute('aria-expanded', 'true');
    await user.click(within(ask).getByRole('button', { name: 'Keep downloading' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(commands.mangaRemoveDownloads).not.toHaveBeenCalled();

    await user.click(ring);
    await user.click(
      within(await screen.findByRole('dialog', { name: 'Stop this download?' })).getByRole(
        'button',
        { name: 'Stop' },
      ),
    );
    expect(commands.mangaRemoveDownloads).toHaveBeenCalledWith(['c1']);
  });

  it('asks the same way for a queued chapter and an episode', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <>
        <DownloadButton chapter={chapter('queued')} />
        <VideoDownloadControl
          item={episode}
          downloaded={false}
          name="Big Buck Bunny"
          onDownload={() => undefined}
        />
      </>,
    );
    await user.click(
      screen.getByRole('button', { name: 'Cancel the queued download of Chapter 1' }),
    );
    expect(await screen.findByRole('dialog', { name: 'Stop this download?' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Keep downloading' }));

    await user.click(
      screen.getByRole('button', { name: 'Downloading Big Buck Bunny, 25%. Cancel' }),
    );
    const ask = await screen.findByRole('dialog', { name: 'Stop this download?' });
    expect(commands.videoDownloadRemove).not.toHaveBeenCalled();
    await user.click(within(ask).getByRole('button', { name: 'Stop' }));
    expect(commands.videoDownloadRemove).toHaveBeenCalledWith('d1');
    expect(commands.mangaRemoveDownloads).not.toHaveBeenCalled();
  });
});
