import type { Settings, StremioItem } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';
import { usePreviewStore } from './cardPreviewStore';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const item = (id: string, name: string, extra: Partial<StremioItem> = {}): StremioItem => ({
  kind: 'movie',
  id,
  name,
  poster: `https://images.example/${id}.jpg`,
  posterShape: 'poster',
  releaseInfo: '2010',
  rating: null,
  playable: true,
  ...extra,
});

/** A mouse that rests on things (desktop), or a finger. */
const pointer = (hovers: boolean) => {
  window.matchMedia = ((query: string) => ({
    matches: hovers && query.includes('hover: hover'),
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
};

const withSettings = (change: Partial<Settings>) =>
  commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, ...change }));

const search = async (items: StremioItem[]) => {
  commands.stremioCatalogs.mockResolvedValue(
    ok([
      {
        addonId: 'cinemeta',
        addonName: 'Cinemeta',
        kind: 'movie',
        catalogId: 'top',
        name: 'Popular',
        extras: [],
        board: false,
        searchable: true,
        discoverable: true,
        pages: true,
        failing: false,
      },
    ]),
  );
  commands.stremioSearchCatalog.mockResolvedValue(
    ok({
      addonId: 'cinemeta',
      addonName: 'Cinemeta',
      kind: 'movie',
      catalogId: 'top',
      name: 'Popular',
      items,
      genres: [],
      hasMore: false,
      error: null,
    }),
  );
  await AppShellWrapper.mount('/browse?tab=search&type=video&view=movies&q=sintel');
  return screen.findByRole('region', { name: 'Movies' });
};

describe('Video cards', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    usePreviewStore.setState({ active: null });
    pointer(true);
  });
  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('are 16:9: a backdrop when known, a portrait poster whole over a fill of itself', async () => {
    const section = await search([
      item('tt1', 'Sintel', { background: 'https://images.example/tt1-wide.jpg' }),
      item('tt2', 'Tears of Steel'),
    ]);
    const wide = await within(section).findByRole('link', { name: 'Sintel' });
    expect(within(wide).queryByTestId('video-cover-fill')).not.toBeInTheDocument();
    expect(wide.querySelector('img[src="https://images.example/tt1-wide.jpg"]')).not.toBeNull();
    const poster = within(section).getByRole('link', { name: 'Tears of Steel' });
    expect(within(poster).getByTestId('video-cover-fill')).toHaveClass('aspect-video');
  });

  it('follow the chosen view: posters, or a compact list', async () => {
    const user = userEvent.setup();
    let stored: Settings = {
      ...DEFAULT_SETTINGS,
      videoCards: { style: 'landscape', previews: 'auto' },
    };
    commands.settingsGet.mockImplementation(async () => ok(stored));
    commands.settingsUpdate.mockImplementation(async (change: { key: string; value: unknown }) => {
      stored = { ...stored, [change.key]: change.value };
      return ok(stored);
    });
    const section = await search([item('tt2', 'Tears of Steel')]);
    await within(section).findByTestId('video-cover-fill');

    await user.click(screen.getByRole('button', { name: 'View' }));
    const sheet = await screen.findByRole('dialog', { name: 'Video titles' });
    await user.click(within(sheet).getByRole('button', { name: 'Portrait posters' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'videoCards',
      value: { style: 'poster', previews: 'auto' },
    });
    await waitFor(() =>
      expect(within(section).queryByTestId('video-cover-fill')).not.toBeInTheDocument(),
    );
  });

  it('play a muted trailer while the mouse rests on one, one at a time', async () => {
    const user = userEvent.setup();
    const section = await search([
      item('tt1', 'Sintel', { trailer: 'yt-sintel' }),
      item('tt2', 'Tears of Steel', { trailer: 'yt-tears' }),
    ]);
    const first = await within(section).findByRole('link', { name: 'Sintel' });
    await user.hover(first);
    await waitFor(() => expect(within(first).getByTestId('card-preview')).toBeInTheDocument());
    expect(within(first).getByTestId('card-preview').querySelector('iframe')).toHaveAttribute(
      'src',
      'about:blank#embed',
    );

    const second = within(section).getByRole('link', { name: 'Tears of Steel' });
    await user.unhover(first);
    await user.hover(second);
    await waitFor(() => expect(within(second).getByTestId('card-preview')).toBeInTheDocument());
    expect(within(first).queryByTestId('card-preview')).not.toBeInTheDocument();
  });

  it('play nothing while previews are off', async () => {
    const user = userEvent.setup();
    withSettings({ videoCards: { style: 'landscape', previews: 'off' } });
    const section = await search([item('tt1', 'Sintel', { trailer: 'yt-sintel' })]);
    const card = await within(section).findByRole('link', { name: 'Sintel' });
    await user.hover(card);
    await new Promise((resolve) => setTimeout(resolve, 800));
    expect(screen.queryByTestId('card-preview')).not.toBeInTheDocument();
  });

  it('play nothing on a touch screen with the default setting', async () => {
    const user = userEvent.setup();
    pointer(false);
    const section = await search([item('tt1', 'Sintel', { trailer: 'yt-sintel' })]);
    const card = await within(section).findByRole('link', { name: 'Sintel' });
    await user.hover(card);
    await new Promise((resolve) => setTimeout(resolve, 800));
    expect(screen.queryByTestId('card-preview')).not.toBeInTheDocument();
  });
});
