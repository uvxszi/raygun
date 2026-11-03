import type { ExtensionInfo, MangaCard, MangaSource, SourceSearchResult } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { rankResults, titleScore } from './rankResults';
import { useSourceHealth } from './sourceHealth';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const source = (id: string, name: string): MangaSource => ({
  id,
  name,
  extensionId: id.split('/')[0] ?? id,
  enabled: true,
});
const card = (id: string, title: string, sourceId: string): MangaCard => ({
  sourceId,
  id,
  title,
  coverUrl: null,
  inLibrary: false,
});
const result = (
  from: MangaSource,
  titles: string[],
  error: string | null = null,
): SourceSearchResult => ({
  source: from,
  items: titles.map((title, index) => card(`${from.id}-${index}`, title, from.id)),
  hasNextPage: false,
  error,
});

const ROYAL = source('x.royal/novel', 'Royal Road');
const SCRIBBLE = source('x.scribble/novel', 'Scribble Hub');
const BROKEN = source('x.broken/novel', 'Broken Site');

describe('Ranked search across sources', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    localStorage.clear();
    useSourceHealth.setState({ failures: {} });
  });

  it('ranks titles by how well they answer the query', () => {
    expect(titleScore('Mother of Learning', 'mother of learning')).toBe(100);
    expect(titleScore('Mother of Learning: Arc 2', 'mother of learning')).toBe(80);
    expect(titleScore('The Learning Mother', 'mother learning')).toBe(60);
    expect(titleScore('Unrelated', 'mother learning')).toBe(0);

    const ranked = rankResults(
      [
        result(ROYAL, ['A Learning Tale', 'Mother of Learning']),
        result(SCRIBBLE, ['Mother of Learning (fan sequel)']),
      ],
      'mother of learning',
    );
    expect(ranked.map((entry) => [entry.card.title, entry.sourceName])).toEqual([
      ['Mother of Learning', 'Royal Road'],
      ['Mother of Learning (fan sequel)', 'Scribble Hub'],
      ['A Learning Tale', 'Royal Road'],
    ]);
    // With nothing typed, the sources take turns.
    expect(
      rankResults([result(ROYAL, ['R1', 'R2']), result(SCRIBBLE, ['S1', 'S2'])], '').map(
        (entry) => entry.card.title,
      ),
    ).toEqual(['R1', 'S1', 'R2', 'S2']);
  });

  it('shows best matches with their sources, then groups by source with failing ones last', async () => {
    const user = userEvent.setup();
    commands.novelSources.mockResolvedValue(ok([BROKEN, ROYAL, SCRIBBLE]));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources) => {
      const id = sources[0];
      if (id === ROYAL.id) return ok([result(ROYAL, ['A Learning Tale', 'Mother of Learning'])]);
      if (id === SCRIBBLE.id) return ok([result(SCRIBBLE, ['Mother of Learning (fan sequel)'])]);
      return ok([result(BROKEN, [], 'HTTP 403')]);
    });
    await AppShellWrapper.mount('/browse?tab=search&type=novel&q=mother%20of%20learning');
    const ranked = await screen.findByTestId('ranked-results');
    await vi.waitFor(() =>
      expect(
        within(ranked)
          .getAllByTestId('manga-card')
          .map((item) => item.textContent),
      ).toEqual([
        'Mother of LearningRoyal Road',
        'Mother of Learning (fan sequel)Scribble Hub',
        'A Learning TaleRoyal Road',
      ]),
    );
    expect(
      within(ranked).getByText(/Couldn't search Broken Site \(HTTP 403\)/),
    ).toBeInTheDocument();

    // Relevant is the default chip (M46.35); All sources groups by source, the failing one last.
    expect(screen.getByRole('button', { name: 'Relevant' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await user.click(screen.getByRole('button', { name: 'All sources' }));
    const rows = await screen.findAllByTestId('source-row');
    await vi.waitFor(() =>
      expect(rows.map((row) => within(row).getByRole('region').getAttribute('aria-label'))).toEqual(
        ['Royal Road', 'Scribble Hub', 'Broken Site'],
      ),
    );
  });

  it("opens a source's website and clears its cookies and data from its menu", async () => {
    const user = userEvent.setup();
    const royal: ExtensionInfo = {
      id: 'x.royal',
      name: 'Royal Road',
      version: '1.0.0',
      description: null,
      contentRating: 'safe',
      origin: 'installed',
      status: 'loaded',
      error: null,
      provides: ['novel.source'],
      net: ['*'],
      providers: [],
      settings: [],
      compatibility: null,
      icon: null,
      website: 'https://www.royalroad.com',
    };
    commands.extensionsList.mockResolvedValue(ok([royal]));
    commands.novelSources.mockResolvedValue(ok([ROYAL]));
    commands.catalogGlobalSearch.mockResolvedValue(ok([result(ROYAL, ['Mother of Learning'])]));
    await AppShellWrapper.mount('/browse?tab=search&type=novel&q=mother');
    await user.click(await screen.findByRole('button', { name: 'Royal Road options' }));
    await user.click(await screen.findByRole('button', { name: 'Open the website' }));
    expect(commands.extensionOpenWebsite).toHaveBeenCalledWith('x.royal');

    await user.click(screen.getByRole('button', { name: 'Royal Road options' }));
    await user.click(await screen.findByRole('button', { name: 'Clear cookies and data' }));
    expect(commands.extensionClearData).toHaveBeenCalledWith('x.royal');
    expect(await screen.findByRole('button', { name: 'Cookies and data cleared' })).toBeDisabled();
  });

  it('finds a source chip by name when there are many', async () => {
    const user = userEvent.setup();
    const many = Array.from({ length: 14 }, (_, index) =>
      source(`x.s${index}/novel`, index === 9 ? 'Zeta Novels' : `Source ${index}`),
    );
    commands.novelSources.mockResolvedValue(ok(many));
    commands.catalogGlobalSearch.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/browse?tab=search&type=novel');
    const chips = await screen.findByRole('group', { name: 'Sources' });
    await user.type(within(chips).getByRole('searchbox', { name: 'Find a source' }), 'zeta');
    expect(within(chips).getByRole('button', { name: 'Zeta Novels' })).toBeInTheDocument();
    expect(within(chips).queryByRole('button', { name: 'Source 1' })).not.toBeInTheDocument();
  });
});
