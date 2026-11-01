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

  it('is in the navigation and plays a link with subtitles', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    await user.click(await screen.findByTestId('nav-watch-tab'));
    const page = await screen.findByTestId('watch-page');

    await user.type(
      within(page).getByRole('textbox', { name: 'Video address' }),
      'https://cdn.example/show.m3u8',
    );
    await user.type(
      within(page).getByRole('textbox', { name: 'Subtitle address' }),
      'https://cdn.example/show.srt',
    );
    await user.click(within(page).getByRole('button', { name: 'Play' }));
    expect(commands.videoPlayLink).toHaveBeenCalledWith(
      'https://cdn.example/show.m3u8',
      null,
      'https://cdn.example/show.srt',
    );
  });

  it('continues, removes and refreshes unfinished videos', async () => {
    const user = userEvent.setup();
    commands.videoContinueWatching.mockResolvedValue(ok([FILM]));
    await AppShellWrapper.mount('/watch');
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
