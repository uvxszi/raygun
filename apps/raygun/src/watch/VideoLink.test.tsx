import type { ContinueWatching, VideoLinkMatch, VideoLinkPage } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { resetBackLayers } from '../shell/backLayers';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const KEY = 'url:magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel';

/** Sintel played from a magnet link (Open link), its files known. */
const SINTEL: VideoLinkPage = {
  key: KEY,
  url: KEY.slice('url:'.length),
  title: 'Sintel',
  searchTitle: 'Sintel',
  poster: 'http://127.0.0.1:1/local/links/08ada5.jpg',
  kind: 'torrent',
  host: null,
  format: 'MP4',
  name: 'Sintel',
  files: [
    {
      index: 2,
      path: 'Sintel/Sintel.mp4',
      name: 'Sintel.mp4',
      sizeBytes: 129_241_752,
      kind: 'video',
      main: true,
      language: null,
    },
    {
      index: 4,
      path: 'Sintel/Sintel.sample.mp4',
      name: 'Sintel.sample.mp4',
      sizeBytes: 3_000_000,
      kind: 'video',
      main: false,
      language: null,
    },
    {
      index: 1,
      path: 'Sintel/Sintel.en.srt',
      name: 'Sintel.en.srt',
      sizeBytes: 1_514,
      kind: 'subtitle',
      main: false,
      language: 'en',
    },
    {
      index: 3,
      path: 'Sintel/poster.jpg',
      name: 'poster.jpg',
      sizeBytes: 46_115,
      kind: 'image',
      main: false,
      language: null,
    },
  ],
  sizeBytes: 129_241_752,
  positionMs: 60_000,
  durationMs: 888_000,
  watched: false,
  matched: null,
  canDownload: true,
};

const MATCH: VideoLinkMatch = {
  kind: 'movie',
  id: 'tt1727587',
  name: 'Sintel',
  year: '2010',
  poster: 'https://images.example/sintel.jpg',
};

const mountPage = async (page: VideoLinkPage) => {
  commands.videoLinkPage.mockResolvedValue(ok(page));
  await AppShellWrapper.mount(`/watch/link?key=${encodeURIComponent(page.key)}`);
  return screen.findByTestId('video-link-page');
};

describe('A link played on its own', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    resetBackLayers(false);
  });

  it("has a page with the torrent's files, each video playable on its own", async () => {
    const user = userEvent.setup();
    const page = await mountPage(SINTEL);
    expect(await within(page).findByRole('heading', { name: 'Sintel' })).toBeInTheDocument();
    expect(commands.videoLinkPage).toHaveBeenCalledWith(KEY);

    const files = within(page).getByRole('region', { name: /Files/ });
    expect(within(files).getByRole('listitem', { name: 'Sintel.mp4' })).toHaveTextContent(
      'Plays first',
    );
    expect(within(files).getByRole('listitem', { name: 'Sintel.en.srt' })).toHaveTextContent(
      'Subtitles · en',
    );
    expect(within(files).getByRole('listitem', { name: 'poster.jpg' })).toHaveTextContent(
      'Picture',
    );
    // Subtitles and pictures don't play on their own.
    expect(within(files).queryByRole('button', { name: 'Play Sintel.en.srt' })).toBeNull();

    // Resume plays it through its session (Sources and Quality in the player).
    await user.click(within(page).getByRole('button', { name: 'Resume' }));
    expect(commands.videoLinkPlay).toHaveBeenCalledWith(KEY, null, false);
    await user.click(within(page).getByRole('button', { name: 'Play from the start' }));
    expect(commands.videoLinkPlay).toHaveBeenLastCalledWith(KEY, null, true);
    await user.click(within(files).getByRole('button', { name: 'Play Sintel.sample.mp4' }));
    expect(commands.videoLinkPlay).toHaveBeenLastCalledWith(KEY, 4, false);

    // Downloads: the video it opens, or one file of it.
    await user.click(within(page).getByRole('button', { name: 'Download Sintel' }));
    await waitFor(() => expect(commands.videoLinkDownload).toHaveBeenCalledWith(KEY, null));
    expect(await screen.findByText('Downloading Sintel')).toBeInTheDocument();
    await user.click(within(files).getByRole('button', { name: 'Download Sintel.sample.mp4' }));
    await waitFor(() => expect(commands.videoLinkDownload).toHaveBeenLastCalledWith(KEY, 4));
  });

  it('asks peers for the files when they are not known yet', async () => {
    const user = userEvent.setup();
    commands.videoLinkFiles.mockResolvedValue(ok(SINTEL));
    const page = await mountPage({ ...SINTEL, files: null });
    const files = within(page).getByRole('region', { name: 'Files' });
    expect(within(files).queryByRole('listitem')).toBeNull();
    await user.click(within(files).getByRole('button', { name: 'Show its files' }));
    expect(commands.videoLinkFiles).toHaveBeenCalledWith(KEY);
    expect(await within(files).findByRole('listitem', { name: 'Sintel.mp4' })).toBeInTheDocument();
  });

  it('is renamed, matched with a movie, and removed after asking', async () => {
    const user = userEvent.setup();
    commands.videoLinkSuggestions.mockResolvedValue(ok([MATCH]));
    const page = await mountPage(SINTEL);

    await user.click(within(page).getByRole('button', { name: 'Rename' }));
    const sheet = await screen.findByRole('dialog', { name: 'Rename' });
    const field = within(sheet).getByRole('textbox', { name: 'Name' });
    await user.clear(field);
    await user.type(field, 'Sintel (2010)');
    await user.click(within(sheet).getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(commands.videoLinkRename).toHaveBeenCalledWith(KEY, 'Sintel (2010)'),
    );

    // The match is optional: suggestions come when asked for.
    const what = within(page).getByRole('region', { name: 'What it is' });
    expect(commands.videoLinkSuggestions).not.toHaveBeenCalled();
    await user.click(within(what).getByRole('button', { name: 'Find what it is' }));
    expect(commands.videoLinkSuggestions).toHaveBeenCalledWith(KEY);
    await user.click(await within(what).findByRole('button', { name: "It's Sintel" }));
    await waitFor(() => expect(commands.videoLinkMatch).toHaveBeenCalledWith(KEY, MATCH));

    // Removing asks first, then leaves the page.
    await user.click(within(page).getByRole('button', { name: 'Options for Sintel' }));
    await user.click(screen.getByRole('menuitem', { name: 'Remove' }));
    expect(commands.videoRemove).not.toHaveBeenCalled();
    const ask = screen.getByRole('alertdialog', {
      name: 'Remove Sintel from Continue watching and your watch history?',
    });
    await user.click(within(ask).getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(commands.videoRemove).toHaveBeenCalledWith(KEY));
  });

  it('shows a matched movie, which opens its page or is forgotten', async () => {
    const user = userEvent.setup();
    const page = await mountPage({ ...SINTEL, matched: MATCH });
    const what = within(page).getByRole('region', { name: 'What it is' });
    expect(within(what).getByRole('link', { name: 'Open its page' })).toHaveAttribute(
      'href',
      '/watch/title?type=movie&id=tt1727587',
    );
    await user.click(within(what).getByRole('button', { name: 'Not this' }));
    await waitFor(() => expect(commands.videoLinkMatch).toHaveBeenCalledWith(KEY, null));
  });

  it('shows a web address and its host, with no files', async () => {
    const page = await mountPage({
      ...SINTEL,
      key: 'url:https://cdn.example/films/sintel.mp4',
      url: 'https://cdn.example/films/sintel.mp4',
      kind: 'file',
      host: 'cdn.example',
      name: null,
      files: null,
      sizeBytes: null,
      poster: null,
    });
    expect(within(page).queryByRole('region', { name: 'Files' })).toBeNull();
    expect(within(page).getByText('https://cdn.example/films/sintel.mp4')).toBeInTheDocument();
    expect(within(page).getByText(/cdn\.example ·|· cdn\.example/)).toBeInTheDocument();
  });

  it('says so when the link was removed', async () => {
    await AppShellWrapper.mount(`/watch/link?key=${encodeURIComponent(KEY)}`);
    expect(await screen.findByText('This link was removed')).toBeInTheDocument();
  });
});

describe("A link's Continue watching card", () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    resetBackLayers(false);
  });

  const LINK: ContinueWatching = {
    key: KEY,
    title: 'Sintel',
    subtitle: null,
    poster: null,
    positionMs: 60_000,
    durationMs: 888_000,
    updatedAt: 1,
    watched: false,
    link: true,
  };

  it('opens its page, from the card and from ⋮', async () => {
    const user = userEvent.setup();
    commands.videoContinueWatching.mockResolvedValue(ok([LINK]));
    commands.videoLinkPage.mockResolvedValue(ok(SINTEL));
    await AppShellWrapper.mount('/library?media=video');
    const row = await screen.findByRole('region', { name: 'Continue watching' });
    const card = within(row).getByRole('listitem', { name: 'Sintel' });
    await user.click(within(card).getByRole('button', { name: 'Options for Sintel' }));
    await user.click(screen.getByRole('menuitem', { name: 'Open page' }));
    expect(await screen.findByTestId('video-link-page')).toBeInTheDocument();
    expect(commands.videoLinkPage).toHaveBeenCalledWith(KEY);
  });
});
