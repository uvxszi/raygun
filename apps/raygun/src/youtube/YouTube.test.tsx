import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { ANDROID_INFO, bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const ADDRESS = 'https://youtu.be/dQw4w9WgXcQ';
const LINK = {
  kind: 'video' as const,
  id: 'dQw4w9WgXcQ',
  url: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
  title: 'Never Gonna Give You Up',
  author: 'Rick Astley',
  thumbnailUrl: 'https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg',
};

describe('YouTube addresses (M46.47)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.youtubeLink.mockResolvedValue(ok(LINK));
  });

  it('plays an address typed in Search, and downloads it with yt-dlp on a computer', async () => {
    const user = userEvent.setup();
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.ytdlpStatus.mockResolvedValue(
      ok({ supported: true, version: null, ffmpeg: false, installing: false, message: null }),
    );
    await AppShellWrapper.mount(`/browse?tab=search&q=${encodeURIComponent(ADDRESS)}`);

    const card = await screen.findByTestId('youtube-link');
    expect(commands.youtubeLink).toHaveBeenCalledWith(ADDRESS);
    expect(card).toHaveTextContent('Never Gonna Give You Up');
    expect(card).toHaveTextContent('Rick Astley');
    expect(
      await within(card).findByText(/yt-dlp is fetched from its GitHub releases/),
    ).toBeInTheDocument();

    await user.click(within(card).getByRole('button', { name: 'Play' }));
    expect(commands.youtubePlay).toHaveBeenCalledWith(
      'dQw4w9WgXcQ',
      'Never Gonna Give You Up',
      'Rick Astley',
    );

    await user.click(within(card).getByRole('button', { name: 'Download video' }));
    expect(commands.ytdlpDownload).toHaveBeenCalledWith(
      LINK.url,
      'Never Gonna Give You Up',
      'video',
    );
    commands.ytdlpJobs.mockResolvedValue([
      {
        id: 'job',
        url: LINK.url,
        title: 'Never Gonna Give You Up',
        kind: 'video',
        state: 'downloading',
        progress: 0.25,
        file: null,
        message: null,
      },
    ]);
    emitEvent('ytDlpEvent', 'job');
    const job = await within(card).findByTestId('ytdlp-job');
    expect(within(job).getByRole('progressbar')).toHaveAttribute('aria-valuenow', '25');
  });

  it('offers no downloads on a phone, and still plays', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount(`/browse?tab=search&q=${encodeURIComponent(ADDRESS)}`);

    const card = await screen.findByTestId('youtube-link');
    await vi.waitFor(() => expect(commands.ytdlpStatus).toHaveBeenCalled());
    expect(within(card).queryByText(/yt-dlp|FFmpeg/)).not.toBeInTheDocument();
    expect(within(card).queryByRole('button', { name: 'Download video' })).not.toBeInTheDocument();
    await user.click(within(card).getByRole('button', { name: 'Play as music' }));
    expect(commands.playbackReplace).toHaveBeenCalled();
  });

  it('tries a failed download again, with what went wrong under More info', async () => {
    const user = userEvent.setup();
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.ytdlpStatus.mockResolvedValue(
      ok({ supported: true, version: '2026.10.1', ffmpeg: true, installing: false, message: null }),
    );
    const failed = {
      id: 'job',
      url: LINK.url,
      title: 'Never Gonna Give You Up',
      kind: 'audio' as const,
      state: 'failed' as const,
      progress: null,
      file: null,
      message: '[youtube] dQw4w9WgXcQ: Video unavailable',
    };
    commands.ytdlpJobs.mockResolvedValue([failed]);
    await AppShellWrapper.mount('/downloads?tab=exports');
    const job = await screen.findByTestId('ytdlp-job');
    const issue = within(job).getByTestId('source-issue');
    expect(issue).toHaveTextContent("Couldn't download from YouTube");
    expect(within(issue).getByText(/Video unavailable/)).not.toBeVisible();

    commands.ytdlpDownload.mockResolvedValue(ok('job-2'));
    await user.click(within(job).getByRole('button', { name: 'Try again' }));
    expect(commands.ytdlpDownload).toHaveBeenCalledWith(
      LINK.url,
      'Never Gonna Give You Up',
      'audio',
    );
    // The failed one leaves the list for the new one.
    await vi.waitFor(() => expect(commands.ytdlpCancel).toHaveBeenCalledWith('job'));

    // A job's event reads the jobs again, not yt-dlp's status.
    commands.ytdlpStatus.mockClear();
    commands.ytdlpJobs.mockClear();
    emitEvent('ytDlpEvent', 'job-2');
    await vi.waitFor(() => expect(commands.ytdlpJobs).toHaveBeenCalled());
    expect(commands.ytdlpStatus).not.toHaveBeenCalled();
  });

  it('leaves ordinary searches alone', async () => {
    await AppShellWrapper.mount('/browse?tab=search&q=daft%20punk');
    await screen.findByTestId('search-page');
    expect(commands.youtubeLink).not.toHaveBeenCalled();
    expect(screen.queryByTestId('youtube-link')).not.toBeInTheDocument();
  });
});
