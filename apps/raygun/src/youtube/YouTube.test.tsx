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
  title: 'Big Buck Bunny',
  author: 'Blender Foundation',
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
    expect(card).toHaveTextContent('Big Buck Bunny');
    expect(card).toHaveTextContent('Blender Foundation');
    expect(
      await within(card).findByText(/yt-dlp is fetched from its GitHub releases/),
    ).toBeInTheDocument();

    await user.click(within(card).getByRole('button', { name: 'Play' }));
    expect(commands.youtubePlay).toHaveBeenCalledWith(
      'dQw4w9WgXcQ',
      'Big Buck Bunny',
      'Blender Foundation',
    );

    await user.click(within(card).getByRole('button', { name: 'Download video' }));
    expect(commands.ytdlpDownload).toHaveBeenCalledWith(LINK.url, 'Big Buck Bunny', 'video');
    commands.ytdlpJobs.mockResolvedValue([
      {
        id: 'job',
        url: LINK.url,
        title: 'Big Buck Bunny',
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

  it('downloads the sound as a song on a phone, and still plays', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount(`/browse?tab=search&q=${encodeURIComponent(ADDRESS)}`);

    const card = await screen.findByTestId('youtube-link');
    await vi.waitFor(() => expect(commands.ytdlpStatus).toHaveBeenCalled());
    expect(within(card).queryByText(/yt-dlp|FFmpeg/)).not.toBeInTheDocument();
    expect(within(card).queryByRole('button', { name: 'Download video' })).not.toBeInTheDocument();
    expect(within(card).queryByText(/needs Raygun on a computer/)).not.toBeInTheDocument();
    await user.click(await within(card).findByRole('button', { name: 'Download' }));
    expect(commands.songsDownload).toHaveBeenCalledWith([
      {
        title: 'Big Buck Bunny',
        artist: 'Blender Foundation',
        album: null,
        durationMs: null,
        artworkUrl: LINK.thumbnailUrl,
        source: { kind: 'youTube', videoId: 'dQw4w9WgXcQ' },
      },
    ]);
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
      title: 'Big Buck Bunny',
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
    expect(issue).toHaveTextContent("Couldn't download this video");
    expect(within(issue).getByText(/Video unavailable/)).not.toBeVisible();

    commands.ytdlpDownload.mockResolvedValue(ok('job-2'));
    await user.click(within(job).getByRole('button', { name: 'Try again' }));
    expect(commands.ytdlpDownload).toHaveBeenCalledWith(LINK.url, 'Big Buck Bunny', 'audio');
    // The failed one leaves the list for the new one.
    await vi.waitFor(() => expect(commands.ytdlpCancel).toHaveBeenCalledWith('job'));

    // A job's event reads the jobs again, not yt-dlp's status.
    commands.ytdlpStatus.mockClear();
    commands.ytdlpJobs.mockClear();
    emitEvent('ytDlpEvent', 'job-2');
    await vi.waitFor(() => expect(commands.ytdlpJobs).toHaveBeenCalled());
    expect(commands.ytdlpStatus).not.toHaveBeenCalled();
  });

  it('points a playlist address to the extensions when no installed source opens it', async () => {
    commands.youtubeLink.mockResolvedValue(
      ok({ ...LINK, kind: 'playlist' as const, id: 'PL123', title: 'Open films' }),
    );
    await AppShellWrapper.mount(`/browse?tab=search&q=${encodeURIComponent(ADDRESS)}`);
    const card = await screen.findByTestId('youtube-link');
    expect(card).toHaveTextContent('Playlist');
    expect(card).not.toHaveTextContent(/YouTube/);
    expect(
      await within(card).findByText(
        'No installed source opens playlists and channels from this address.',
      ),
    ).toBeInTheDocument();
    expect(within(card).getByRole('link', { name: 'Find extensions' })).toHaveAttribute(
      'href',
      expect.stringContaining('tab=extensions'),
    );
  });

  it("opens a playlist's songs when a music source reads its address", async () => {
    commands.youtubeLink.mockResolvedValue(
      ok({ ...LINK, kind: 'playlist' as const, id: 'PL123', title: 'Open films' }),
    );
    commands.musicSearchPlaylists.mockResolvedValue(
      ok([
        {
          id: 'PL123',
          title: 'Open films',
          author: 'Blender Foundation',
          artworkUrl: null,
          trackCount: 17,
          featured: false,
          provider: {
            id: 'nuclear.nuclear-plugin-youtube-playlists/youtube-playlists',
            extensionId: 'nuclear.nuclear-plugin-youtube-playlists',
            kind: 'music.metadata',
            name: 'YouTube Playlists',
            adult: false,
          },
        },
      ]),
    );
    await AppShellWrapper.mount(`/browse?tab=search&q=${encodeURIComponent(ADDRESS)}`);
    const card = await screen.findByTestId('youtube-link');
    const open = await within(card).findByRole('link', {
      name: 'Open its songs (YouTube Playlists)',
    });
    expect(open).toHaveAttribute('href', expect.stringContaining('/music/playlist'));
    expect(open).toHaveAttribute('href', expect.stringContaining('id=PL123'));
    expect(within(card).queryByText(/No installed source/)).not.toBeInTheDocument();
  });

  it('leaves ordinary searches alone', async () => {
    await AppShellWrapper.mount('/browse?tab=search&q=daft%20punk');
    await screen.findByTestId('search-page');
    expect(commands.youtubeLink).not.toHaveBeenCalled();
    expect(screen.queryByTestId('youtube-link')).not.toBeInTheDocument();
  });
});
