import type {
  MangaCard,
  MangaSource,
  OnlineTrack,
  SettingChange,
  Settings,
  SourceSearchResult,
} from '@raygun/bindings';
import { act, fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const source = (id: string, name: string): MangaSource => ({
  id,
  name,
  extensionId: id.split('/')[0] ?? id,
  enabled: true,
});
const card = (id: string, title: string, sourceId: string, fromListing = false): MangaCard => ({
  sourceId,
  id,
  title,
  coverUrl: null,
  inLibrary: false,
  ...(fromListing ? { fromListing } : {}),
});
const song = (title: string, artist: string): OnlineTrack => ({
  externalId: null,
  title,
  artist,
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
});

const ROYAL = source('x.royal/novel', 'Royal Road');
const SCRIBBLE = source('x.scribble/novel', 'Scribble Hub');

describe('Sources whose search fails or finds nothing', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    localStorage.clear();
  });

  it('are looked through by default, as many matches and pages as Browse settings say', async () => {
    const user = userEvent.setup();
    const shown = (await commands.settingsGet()) as { data: Settings };
    commands.settingsUpdate.mockImplementation(async (change: SettingChange) =>
      ok({ ...shown.data, [change.key]: change.value }),
    );
    await AppShellWrapper.mount('/settings/browse');

    const toggle = await screen.findByRole('switch', {
      name: 'Look through sources whose search fails',
    });
    expect(toggle).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByRole('slider', { name: 'Matches to collect' })).toHaveValue('20');
    const pages = screen.getByRole('slider', { name: 'Pages to look through' });
    expect(pages).toHaveValue('5');

    // Saved once let go (the slider's `change`).
    await act(async () => {
      fireEvent.change(pages, { target: { value: '8' } });
    });
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'searchListings',
      value: { enabled: true, entries: 20, pages: 8 },
    });
    await user.click(toggle);
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'searchListings',
      value: { enabled: false, entries: 20, pages: 8 },
    });
    // Off, how much to read doesn't apply.
    expect(toggle).toHaveAttribute('aria-checked', 'false');
    expect(screen.getByRole('slider', { name: 'Pages to look through' })).toBeDisabled();
  });

  it('say beside the source’s name that its titles come from its listings', async () => {
    const user = userEvent.setup();
    commands.novelSources.mockResolvedValue(ok([ROYAL, SCRIBBLE]));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources) => {
      if (sources[0] === SCRIBBLE.id) {
        return ok<SourceSearchResult[]>([
          {
            source: SCRIBBLE,
            items: [card('s1', 'Mother of Learning', SCRIBBLE.id)],
            hasNextPage: false,
            error: null,
          },
        ]);
      }
      return ok<SourceSearchResult[]>([
        {
          source: ROYAL,
          items: [card('r1', 'Mother of Learning 2', ROYAL.id, true)],
          hasNextPage: false,
          error: null,
          fromListings: true,
        },
      ]);
    });
    await AppShellWrapper.mount('/browse?tab=search&type=novel&q=mother%20of%20learning');
    await screen.findByTestId('ranked-results');
    await user.click(screen.getByRole('button', { name: 'All sources' }));

    // Only the source that answered from its listings says so.
    const note = await screen.findByTestId('from-listings');
    expect(screen.getAllByTestId('from-listings')).toHaveLength(1);
    expect(note).toHaveTextContent('From its listings');
    expect(note).toHaveAttribute('title', expect.stringContaining('failed or found nothing'));
    const royal = note.closest('section');
    expect(royal).toHaveAttribute('aria-label', 'Royal Road');
    // Said once for the row, not again on each cover.
    expect(within(royal!).getByTestId('manga-card')).toHaveTextContent('Mother of Learning 2');
    expect(within(royal!).getByTestId('manga-card')).not.toHaveTextContent('From its listings');
  });

  it('mark songs found in a music source’s charts', async () => {
    const BROKEN = {
      id: 'dev.example.broken/songs',
      extensionId: 'dev.example.broken',
      kind: 'music.metadata',
      name: 'Broken Songs',
    };
    const MUSICBRAINZ = { ...BROKEN, id: 'dev.raygun.musicbrainz/recordings', name: 'MusicBrainz' };
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: BROKEN,
            tracks: [song('Get Lucky', 'Daft Punk')],
            error: null,
            fromListings: true,
          },
          { provider: MUSICBRAINZ, tracks: [song('Lucky Star', 'Madonna')], error: null },
        ],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=songs&q=get%20lucky');
    const best = await screen.findByTestId('best-songs');
    const [found, other] = within(best).getAllByTestId('online-track-row');
    expect(found).toHaveTextContent('Get Lucky');
    expect(within(found!).getByTestId('from-listings')).toHaveTextContent('From its listings');
    expect(other).toHaveTextContent('Lucky Star');
    expect(within(other!).queryByTestId('from-listings')).not.toBeInTheDocument();
  });
});
