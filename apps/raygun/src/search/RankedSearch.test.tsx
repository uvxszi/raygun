import type { ExtensionInfo, MangaCard, MangaSource, SourceSearchResult } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { rankResults, titleScore } from './rankResults';
import { answeredFirst, useSourceHealth } from './sourceHealth';
import type { SourceAnswer } from './sourceHealth';
import { MAX_IN_FLIGHT, queued } from './useCatalogSearch';

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
const QUIET = source('x.quiet/novel', 'Quiet Library');

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
    // The failing source is named once, quietly, after the answers; its error is under More info.
    const failed = within(ranked).getByTestId('failed-sources');
    expect(failed).toHaveTextContent('Broken Site refused access');
    expect(failed).toHaveTextContent('HTTP 403');

    // Every source searches while Custom sources is off, best matches first (M46.35); By source
    // gives each its row, the failing one last.
    expect(screen.getByRole('button', { name: /Custom sources/ })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
    expect(screen.getByRole('button', { name: 'Best matches' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await user.click(screen.getByRole('button', { name: 'By source' }));
    const rows = await screen.findAllByTestId('source-row');
    await vi.waitFor(() =>
      expect(rows.map((row) => within(row).getByRole('region').getAttribute('aria-label'))).toEqual(
        ['Royal Road', 'Scribble Hub', 'Broken Site'],
      ),
    );
  });

  it('marks titles found in a source’s listings rather than its search', async () => {
    commands.novelSources.mockResolvedValue(ok([ROYAL, SCRIBBLE]));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources) => {
      if (sources[0] === SCRIBBLE.id) return ok([result(SCRIBBLE, ['Mother of Learning'])]);
      return ok([
        {
          ...result(ROYAL, []),
          items: [{ ...card('listed', 'Mother of Learning 2', ROYAL.id), fromListing: true }],
        },
      ]);
    });
    await AppShellWrapper.mount('/browse?tab=search&type=novel&q=mother%20of%20learning');
    const ranked = await screen.findByTestId('ranked-results');
    await vi.waitFor(() =>
      expect(
        within(ranked)
          .getAllByTestId('manga-card')
          .map((item) => item.textContent),
      ).toEqual([
        'Mother of LearningScribble Hub',
        'Mother of Learning 2From its listingsRoyal Road',
      ]),
    );
  });

  it('orders sources by their answers: results, then none, then failures; unknown ones stay', () => {
    const answers: Record<string, SourceAnswer | undefined> = {
      a: undefined,
      b: 'empty',
      c: 'results',
      d: undefined,
      e: 'failed',
      f: 'results',
    };
    const ids = Object.keys(answers);
    // a and d haven't answered: they keep their places, the others are sorted around them.
    expect(answeredFirst(ids, (id) => answers[id])).toEqual(['a', 'c', 'f', 'd', 'b', 'e']);
    expect(answeredFirst(['x', 'y'], () => undefined)).toEqual(['x', 'y']);
  });

  it('by source, puts sources that found nothing after those with titles and before failures', async () => {
    const user = userEvent.setup();
    commands.novelSources.mockResolvedValue(ok([QUIET, BROKEN, ROYAL]));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources) => {
      const id = sources[0];
      if (id === ROYAL.id) return ok([result(ROYAL, ['Mother of Learning'])]);
      if (id === QUIET.id) return ok([result(QUIET, [])]);
      return ok([result(BROKEN, [], 'HTTP 403')]);
    });
    await AppShellWrapper.mount('/browse?tab=search&type=novel&q=mother%20of%20learning');
    await screen.findByTestId('ranked-results');
    await user.click(screen.getByRole('button', { name: 'By source' }));
    // The answers the best matches already got order the rows at once.
    await vi.waitFor(() =>
      expect(
        screen
          .getAllByTestId('source-row')
          .map((row) => within(row).getByRole('region').getAttribute('aria-label')),
      ).toEqual(['Royal Road', 'Quiet Library', 'Broken Site']),
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
      repository: null,
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

  it("drops a source's search when the query moves on before its turn, and asks once", async () => {
    // Every place taken by searches that answer when released.
    const releases: (() => void)[] = [];
    const held = Array.from({ length: MAX_IN_FLIGHT }, (_, index) =>
      queued(
        `held-${index}`,
        () => new Promise<string>((resolve) => releases.push(() => resolve(`held ${index}`))),
      ),
    );
    const controller = new AbortController();
    const stale = vi.fn(async () => 'stale');
    const waiting = queued('stale', stale, controller.signal);
    const fresh = vi.fn(async () => 'fresh');
    const next = queued('fresh', fresh);
    controller.abort();
    await expect(waiting).rejects.toBeDefined();

    releases[0]!();
    expect(await next).toBe('fresh');
    expect(stale).not.toHaveBeenCalled();
    // The same search asked again while it's being asked waits for that answer.
    let answer: () => void = () => {};
    const slow = vi.fn(
      () =>
        new Promise<string>((resolve) => {
          answer = () => resolve('once');
        }),
    );
    const first = queued('same', slow);
    await vi.waitFor(() => expect(slow).toHaveBeenCalledTimes(1));
    const second = queued('same', slow);
    answer();
    expect(await Promise.all([first, second])).toEqual(['once', 'once']);
    expect(slow).toHaveBeenCalledTimes(1);
    releases.forEach((release) => release());
    await Promise.all(held);
  });

  it('finds a source by name in Custom sources when there are many, folded after a dozen', async () => {
    const user = userEvent.setup();
    const many = Array.from({ length: 14 }, (_, index) =>
      source(`x.s${index}/novel`, index === 12 ? 'Zeta Novels' : `Source ${index}`),
    );
    commands.novelSources.mockResolvedValue(ok(many));
    commands.catalogGlobalSearch.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/browse?tab=search&type=novel');
    await user.click(await screen.findByRole('button', { name: /Custom sources/ }));
    const panel = await screen.findByRole('group', { name: 'Sources' });
    expect(within(panel).getAllByRole('switch')).toHaveLength(12);
    expect(within(panel).queryByRole('switch', { name: 'Zeta Novels' })).not.toBeInTheDocument();
    await user.type(within(panel).getByRole('searchbox', { name: 'Find a source' }), 'zeta');
    expect(within(panel).getByRole('switch', { name: 'Zeta Novels' })).toBeInTheDocument();
    expect(within(panel).queryByRole('switch', { name: 'Source 1' })).not.toBeInTheDocument();

    await user.clear(within(panel).getByRole('searchbox', { name: 'Find a source' }));
    await user.click(within(panel).getByRole('button', { name: '2 more' }));
    expect(within(panel).getAllByRole('switch')).toHaveLength(14);
  });
});
