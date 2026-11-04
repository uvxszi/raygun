import type { MangaCard, OnlineTrack, Settings } from '@raygun/bindings';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SAFE = {
  id: 'dev.example.safe/safe',
  name: 'Safe',
  extensionId: 'dev.example.safe',
  enabled: true,
};
const ADULT = {
  id: 'dev.example.adult/adult',
  name: 'Grown-up',
  extensionId: 'dev.example.adult',
  enabled: true,
  adult: true,
};
const card = (id: string, title: string, sourceId: string, adult = false): MangaCard => ({
  sourceId,
  id,
  title,
  coverUrl: null,
  inLibrary: false,
  ...(adult ? { adult: true } : {}),
});
const track = (title: string, explicit = false): OnlineTrack => ({
  externalId: title,
  title,
  artist: 'Band',
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
  ...(explicit ? { explicit: true } : {}),
});

/** Settings kept by the mocked commands, starting from `start`. */
const keepSettings = (start: Partial<Settings>) => {
  let stored: Settings = { ...DEFAULT_SETTINGS, ...start };
  commands.settingsGet.mockImplementation(async () => ok(stored));
  commands.settingsUpdate.mockImplementation(async (change: { key: string; value: unknown }) => {
    stored = { ...stored, [change.key]: change.value };
    return ok(stored);
  });
};

describe('Search filter switches', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaSources.mockResolvedValue(ok([SAFE, ADULT]));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources: string[]) =>
      ok(
        sources[0] === ADULT.id
          ? [
              {
                source: ADULT,
                items: [card('a1', 'Frieren Nights', ADULT.id)],
                hasNextPage: false,
                error: null,
              },
            ]
          : [
              {
                source: SAFE,
                items: [card('s1', 'Frieren', SAFE.id), card('s2', 'Frieren Uncut', SAFE.id, true)],
                hasNextPage: false,
                error: null,
              },
            ],
      ),
    );
  });

  it('shows both on and locked while the content settings already hide them', async () => {
    keepSettings({ showNsfw: false, explicitMusic: false });
    await AppShellWrapper.mount('/browse?tab=search&type=manga&q=frieren');
    const noAdult = await screen.findByRole('switch', { name: 'No 18+' });
    const noExplicit = screen.getByRole('switch', { name: 'No explicit' });
    expect(noAdult).toBeChecked();
    expect(noAdult).toBeDisabled();
    expect(noAdult).toHaveAccessibleDescription(/Show NSFW is off/);
    expect(noExplicit).toBeChecked();
    expect(noExplicit).toBeDisabled();
    expect(noExplicit).toHaveAccessibleDescription(/Explicit music is off/);
  });

  it('No 18+ leaves adult sources unasked and adult titles out, and is saved', async () => {
    const user = userEvent.setup();
    keepSettings({ showNsfw: true });
    await AppShellWrapper.mount('/browse?tab=search&type=manga&q=frieren');
    expect(await screen.findByText('Frieren Uncut')).toBeInTheDocument();
    expect(await screen.findByText('Frieren Nights')).toBeInTheDocument();

    const noAdult = screen.getByRole('switch', { name: 'No 18+' });
    expect(noAdult).not.toBeChecked();
    await user.click(noAdult);

    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'searchFilter',
      value: { noAdult: true, noExplicit: false },
    });
    await waitFor(() => expect(screen.queryByText('Frieren Uncut')).not.toBeInTheDocument());
    expect(screen.queryByText('Frieren Nights')).not.toBeInTheDocument();
    expect(screen.getByText('Frieren')).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'No 18+' })).toBeChecked();
    expect(screen.queryByRole('button', { name: 'Grown-up' })).not.toBeInTheDocument();
  });

  it('No explicit leaves explicit songs out', async () => {
    const user = userEvent.setup();
    keepSettings({ showNsfw: true, explicitMusic: true });
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: { id: 'dev.example.music/search', name: 'Tunes', kind: 'music.metadata' },
            tracks: [track('Clean Song'), track('Rude Song', true)],
            error: null,
          },
        ],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=song');
    expect(await screen.findByText('Rude Song')).toBeInTheDocument();

    await user.click(screen.getByRole('switch', { name: 'No explicit' }));
    await waitFor(() => expect(screen.queryByText('Rude Song')).not.toBeInTheDocument());
    expect(screen.getAllByText('Clean Song').length).toBeGreaterThan(0);
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'searchFilter',
      value: { noAdult: false, noExplicit: true },
    });
  });
});
