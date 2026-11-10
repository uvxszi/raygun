import type { MangaTitleView, StremioMeta } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { ANDROID_INFO, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.example.comics/comics';
const COVER = 'http://127.0.0.1:1/cover/frieren';

const TITLE: MangaTitleView = {
  id: 't1',
  hiddenNsfw: false,
  sourceId: SOURCE,
  sourceName: 'Comic Shelf',
  remoteId: 'm1',
  title: 'Frieren',
  coverUrl: COVER,
  authors: ['Yamada'],
  artists: [],
  description: null,
  genres: [],
  status: 'ongoing',
  url: null,
  inLibrary: false,
  chapters: [],
  continueChapterId: null,
} as MangaTitleView;

const MOVIE = {
  continueVideoId: null,
  kind: 'movie',
  id: 'tt0133093',
  name: 'The Matrix',
  poster: 'http://127.0.0.1:1/cover/poster',
  background: 'http://127.0.0.1:1/cover/background',
  logo: null,
  description: null,
  genres: [],
  releaseInfo: '1999',
  runtime: null,
  rating: null,
  ratings: [],
  cast: [],
  director: [],
  trailerYtIds: [],
  videos: [],
  positionMs: 0,
  durationMs: null,
  watched: false,
  downloaded: false,
  addonName: 'Cinemeta',
  addonId: 'cinemeta',
  videoId: 'tt0133093',
  live: false,
  inLibrary: false,
  titleId: null,
} as StremioMeta;

describe('Cover viewer', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaSources.mockResolvedValue(
      ok([{ id: SOURCE, name: 'Comic Shelf', extensionId: 'dev.example.comics' }]),
    );
  });

  it("opens a title's cover at its largest version, zooms, saves, shares and copies it", async () => {
    const user = userEvent.setup();
    commands.mangaTitle.mockResolvedValue(ok(TITLE));
    commands.coverVersions.mockResolvedValue(['http://127.0.0.1:1/cover/large', COVER]);
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(SOURCE)}&id=m1`);
    const page = await screen.findByTestId('manga-title-page');
    await within(page).findByRole('heading', { name: 'Frieren' });

    // A plain click leaves it; a right-click opens it.
    const cover = within(page).getByRole('button', { name: 'Show the cover large' });
    await user.click(cover);
    expect(screen.queryByTestId('cover-sheet')).not.toBeInTheDocument();
    await user.pointer({ keys: '[MouseRight]', target: cover });
    const sheet = await screen.findByTestId('cover-sheet');
    expect(commands.coverVersions).toHaveBeenCalledWith(COVER);
    const picture = await within(sheet).findByRole('img', { name: 'Frieren' });
    expect(picture).toHaveAttribute('src', 'http://127.0.0.1:1/cover/large');
    // A larger version the host doesn't have gives way to the cover itself.
    fireEvent.error(picture);
    expect(await within(sheet).findByRole('img', { name: 'Frieren' })).toHaveAttribute(
      'src',
      COVER,
    );

    const frame = within(sheet).getByTestId('zoomable-image');
    expect(frame).toHaveAttribute('data-scale', '1.00');
    expect(within(sheet).getByRole('button', { name: 'Zoom out' })).toBeDisabled();
    await user.click(within(sheet).getByRole('button', { name: 'Zoom in' }));
    expect(frame).toHaveAttribute('data-scale', '1.50');
    await user.click(within(sheet).getByRole('button', { name: 'Fit to the window' }));
    expect(frame).toHaveAttribute('data-scale', '1.00');
    fireEvent.wheel(frame, { deltaY: -200 });
    expect(Number(frame.getAttribute('data-scale'))).toBeGreaterThan(1);

    await user.click(within(sheet).getByRole('button', { name: 'Save image' }));
    expect(commands.coverSave).toHaveBeenCalledWith(COVER, 'Frieren', 'jpeg', 'pictures');
    await user.click(within(sheet).getByRole('button', { name: 'Share' }));
    expect(commands.coverShare).toHaveBeenCalledWith(COVER, 'Frieren');
    await user.click(within(sheet).getByRole('button', { name: 'Copy image' }));
    expect(commands.coverCopy).toHaveBeenCalledWith(COVER, 'Frieren');
    expect(await within(sheet).findByText('Copied')).toBeInTheDocument();
  });

  it("opens a movie's backdrop on a right-click, and has no Share on desktop", async () => {
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.stremioMeta.mockResolvedValue(ok(MOVIE));
    await AppShellWrapper.mount('/watch/title?type=movie&id=tt0133093');
    const page = await screen.findByTestId('watch-title-page');
    fireEvent.contextMenu(await within(page).findByTestId('title-backdrop'));
    const sheet = await screen.findByTestId('cover-sheet');
    expect(commands.coverVersions).toHaveBeenCalledWith('http://127.0.0.1:1/cover/background');
    expect(
      await within(sheet).findByRole('img', { name: 'The Matrix - Background' }),
    ).toBeInTheDocument();
    expect(within(sheet).queryByRole('button', { name: 'Share' })).not.toBeInTheDocument();
    expect(within(sheet).getByRole('button', { name: 'Copy image' })).toBeInTheDocument();
  });

  it("adds View cover to a grid card's menu", async () => {
    const user = userEvent.setup();
    commands.mangaBrowse.mockResolvedValue(
      ok({
        items: [{ sourceId: SOURCE, id: 'm1', title: 'Frieren', coverUrl: COVER }],
        hasNextPage: false,
      }),
    );
    await AppShellWrapper.mount(`/manga/source?id=${encodeURIComponent(SOURCE)}`);
    const page = await screen.findByTestId('manga-source-page');
    await within(page).findByTestId('manga-card');
    await user.click(within(page).getByRole('button', { name: 'Options for Frieren' }));
    await user.click(await screen.findByRole('menuitem', { name: 'View cover' }));
    const sheet = await screen.findByTestId('cover-sheet');
    expect(await within(sheet).findByRole('img', { name: 'Frieren' })).toHaveAttribute(
      'src',
      COVER,
    );
  });
});
