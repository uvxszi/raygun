import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const FILM = {
  key: 'url:https://cdn.example/film.mp4',
  title: 'A Film',
  subtitle: null,
  poster: null,
  positionMs: 600_000,
  durationMs: 1_200_000,
  updatedAt: 1,
};

describe('Watch tab', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('lives in Library → Videos and plays a link with subtitles from the menu', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library');
    await user.click(await screen.findByTestId('library-media-video'));
    const videos = await screen.findByTestId('videos-library');

    await user.click(within(videos).getByRole('button', { name: 'More options' }));
    await user.click(within(videos).getByRole('menuitem', { name: 'Open link or file' }));
    const sheet = await screen.findByRole('dialog', { name: 'Open link or file' });
    expect(within(sheet).getByRole('radio', { name: 'Video' })).toBeChecked();
    await user.type(
      within(sheet).getByRole('textbox', { name: 'Video address' }),
      'https://cdn.example/show.m3u8',
    );
    await user.type(
      within(sheet).getByRole('textbox', { name: 'Subtitle address' }),
      'https://cdn.example/show.srt',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Play' }));
    expect(commands.videoPlayLink).toHaveBeenCalledWith(
      'https://cdn.example/show.m3u8',
      null,
      'https://cdn.example/show.srt',
    );
    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('continues, removes and refreshes unfinished videos', async () => {
    const user = userEvent.setup();
    commands.videoContinueWatching.mockResolvedValue(ok([FILM]));
    await AppShellWrapper.mount('/library?media=video');
    const item = await screen.findByRole('listitem', { name: 'A Film' });

    await user.click(within(item).getByRole('button', { name: /^A Film/ }));
    expect(commands.videoResume).toHaveBeenCalledWith(FILM.key);

    commands.videoContinueWatching.mockResolvedValue(ok([]));
    await user.click(
      within(item).getByRole('button', { name: 'Remove A Film from Continue watching' }),
    );
    expect(commands.videoRemove).toHaveBeenCalledWith(FILM.key);
    await vi.waitFor(() => expect(screen.queryByText('Continue watching')).not.toBeInTheDocument());

    commands.videoContinueWatching.mockResolvedValue(ok([{ ...FILM, title: 'Next Film' }]));
    emitEvent('watchChangedEvent', FILM.key);
    expect(await screen.findByRole('listitem', { name: 'Next Film' })).toBeInTheDocument();
  });
});
