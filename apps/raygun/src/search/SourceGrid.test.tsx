import type { ExtensionInfo, RepoCatalog } from '@raygun/bindings';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { FC } from 'react';

import { bindingsMock, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { sourceExtension } from '../test/sourceExtension';
import { SourceSwitchPanel } from './CustomSources';
import { useCustomSources } from './useCustomSources';
import type { SourceOption } from './useCustomSources';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const KINDS = ['Comic Shelf', 'Daily Strips', 'Panel Vault', 'Manga Scans'];

/** `count` sources, each from its own extension: "Comic Shelf 0", "Daily Strips 1"… */
const sources = (count: number): SourceOption[] =>
  Array.from({ length: count }, (_, index) => ({
    id: `dev.example.ext${index}/source`,
    name: `${KINDS[index % KINDS.length]} ${index}`,
    extensionId: `dev.example.ext${index}`,
    adult: index === 2,
  }));

/** Their extensions: a third from Paraline, a third from Keiyoushi, a third added by hand. */
const extensions = (count: number): ExtensionInfo[] =>
  Array.from({ length: count }, (_, index) => ({
    ...sourceExtension(['manga.source'], `dev.example.ext${index}`, `Extension ${index}`),
    repository: ['Paraline', 'Keiyoushi', null][index % 3] ?? null,
    icon: index === 0 ? 'http://127.0.0.1:9/cover/ext-0' : null,
  }));

/** The search with its sources' panel, as Search → Comics holds it. */
const Panel: FC<{ options: SourceOption[] }> = ({ options }) => {
  const choice = useCustomSources('manga', options);
  return (
    <SourceSwitchPanel
      open
      id="custom-sources"
      medium="manga"
      sources={options}
      picked={choice.picked}
      onChange={choice.set}
      onChoose={choice.choose}
    />
  );
};

/** Every source on, as turning Custom sources on starts. */
const open = (options: SourceOption[]) => {
  localStorage.setItem(
    'raygun.search.sources.manga',
    JSON.stringify(options.map((option) => option.id)),
  );
  renderWithProviders(<Panel options={options} />);
  return screen.findByTestId('source-grid');
};

/** The panel's width and height, as the grid measures them (jsdom has no layout). */
const panelSize = (width: number, height = 384) => {
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(width);
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(height);
};

const tile = (name: string | RegExp) => screen.getByRole('switch', { name });
const tileNames = (grid: HTMLElement) =>
  within(grid)
    .getAllByRole('switch')
    .map((element) => element.getAttribute('aria-label'));
const onNames = (grid: HTMLElement) =>
  within(grid)
    .getAllByRole('switch')
    .filter((element) => element.getAttribute('aria-checked') === 'true')
    .map((element) => element.getAttribute('aria-label'));

describe("Custom sources' grid", () => {
  beforeEach(() => {
    seedBindings();
    commands.extensionsList.mockResolvedValue(ok(extensions(400)));
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    delete (HTMLElement.prototype as { scrollTo?: unknown }).scrollTo;
  });

  it.each([
    [340, 4],
    [450, 6],
    [640, 8],
  ])('a panel %d px wide has %d tiles a row', async (width, columns) => {
    panelSize(width);
    const grid = await open(sources(10));
    expect(grid).toHaveAttribute('data-columns', String(columns));
    const first = tile('Comic Shelf 0');
    expect(within(first.parentElement!).getAllByRole('switch')).toHaveLength(columns);
  });

  it('follows the panel as it grows and shrinks', async () => {
    const watching: (() => void)[] = [];
    vi.stubGlobal(
      'ResizeObserver',
      class {
        constructor(changed: () => void) {
          watching.push(changed);
        }
        observe() {}
        unobserve() {}
        disconnect() {}
      },
    );
    let width = 340;
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(() => width);
    const grid = await open(sources(10));
    expect(grid).toHaveAttribute('data-columns', '4');
    width = 640;
    act(() => watching.forEach((changed) => changed()));
    expect(grid).toHaveAttribute('data-columns', '8');
  });

  it('shows each source by its icon or letter and name, on or off, and switches one', async () => {
    const user = userEvent.setup();
    const grid = await open(sources(3));
    // The extension's icon, or the source's first letter.
    await waitFor(() =>
      expect(tile('Comic Shelf 0').querySelector('img')).toHaveAttribute(
        'src',
        'http://127.0.0.1:9/cover/ext-0',
      ),
    );
    expect(tile('Comic Shelf 0').querySelector('img')).toHaveAttribute('loading', 'lazy');
    expect(tile('Daily Strips 1').querySelector('img')).toBeNull();
    expect(tile('Daily Strips 1')).toHaveTextContent('D');
    // 18+ sources say so.
    expect(tile('Panel Vault 2, For adults (18+)')).toHaveTextContent('18+');
    expect(onNames(grid)).toHaveLength(3);
    expect(screen.getByText('3 of 3 on')).toBeInTheDocument();

    await user.click(tile('Daily Strips 1'));
    expect(tile('Daily Strips 1')).toHaveAttribute('aria-checked', 'false');
    expect(screen.getByText('2 of 3 on')).toBeInTheDocument();
    // Remembered as Custom sources always was.
    expect(JSON.parse(localStorage.getItem('raygun.search.sources.manga') ?? '[]')).toEqual([
      'dev.example.ext0/source',
      'dev.example.ext2/source',
    ]);
  });

  it('finds sources by name; All, None and Invert switch what is shown', async () => {
    const user = userEvent.setup();
    panelSize(600);
    const grid = await open(sources(20));
    const field = screen.getByRole('searchbox', { name: 'Find a source' });

    await user.type(field, 'scans');
    await waitFor(() => expect(tileNames(grid)).toHaveLength(5));
    expect(tileNames(grid).every((name) => name?.startsWith('Manga Scans'))).toBe(true);
    expect(screen.getByText(/20 of 20 on · 5 matches/)).toBeInTheDocument();

    // None switches off the five shown; the others stay on.
    await user.click(screen.getByRole('button', { name: 'None' }));
    expect(onNames(grid)).toEqual([]);
    expect(screen.getByText(/15 of 20 on/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'All' }));
    expect(screen.getByText(/20 of 20 on/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'None' }));

    // Nothing typed, Invert flips every source.
    await user.clear(field);
    await waitFor(() => expect(tileNames(grid)).toHaveLength(20));
    await user.click(screen.getByRole('button', { name: 'Invert' }));
    expect(onNames(grid)).toEqual([
      'Manga Scans 3',
      'Manga Scans 7',
      'Manga Scans 11',
      'Manga Scans 15',
      'Manga Scans 19',
    ]);
    expect(screen.getByText('5 of 20 on')).toBeInTheDocument();

    await user.type(field, 'no such source');
    expect(await screen.findByText('No sources match.')).toBeInTheDocument();
  });

  it('moves between tiles with the arrow keys and switches one with Space or Enter', async () => {
    const user = userEvent.setup();
    panelSize(340);
    const grid = await open(sources(10));
    // One stop for Tab: the first tile.
    expect(
      within(grid)
        .getAllByRole('switch')
        .filter((element) => element.tabIndex === 0),
    ).toEqual([tile('Comic Shelf 0')]);
    act(() => tile('Comic Shelf 0').focus());

    await user.keyboard('{ArrowRight}');
    expect(tile('Daily Strips 1')).toHaveFocus();
    // Down keeps the column: four tiles a row.
    await user.keyboard('{ArrowDown}');
    expect(tile('Daily Strips 5')).toHaveFocus();
    await user.keyboard('{ArrowDown}');
    // The last row has two tiles: the closest one.
    expect(tile('Comic Shelf 8')).not.toHaveFocus();
    expect(tile('Daily Strips 9')).toHaveFocus();
    await user.keyboard('{ArrowUp}');
    expect(tile('Daily Strips 5')).toHaveFocus();
    await user.keyboard('{End}');
    expect(tile('Manga Scans 7')).toHaveFocus();
    await user.keyboard('{ArrowRight}');
    expect(tile('Comic Shelf 8')).toHaveFocus();
    await user.keyboard('{ArrowLeft}{Home}');
    expect(tile('Comic Shelf 4')).toHaveFocus();
    // Tab comes back to the last tile moved to.
    expect(tile('Comic Shelf 4')).toHaveAttribute('tabindex', '0');
    expect(tile('Comic Shelf 0')).toHaveAttribute('tabindex', '-1');

    await user.keyboard(' ');
    expect(tile('Comic Shelf 4')).toHaveAttribute('aria-checked', 'false');
    await user.keyboard('{Enter}');
    expect(tile('Comic Shelf 4')).toHaveAttribute('aria-checked', 'true');
    await user.keyboard('{Control>}{End}{/Control}');
    expect(tile('Daily Strips 9')).toHaveFocus();
    await user.keyboard('{Control>}{Home}{/Control}');
    expect(tile('Comic Shelf 0')).toHaveFocus();
  });

  it('goes on from the field to the tiles with Down', async () => {
    const user = userEvent.setup();
    panelSize(340);
    await open(sources(20));
    await user.click(screen.getByRole('searchbox', { name: 'Find a source' }));
    await user.keyboard('panel');
    await waitFor(() => expect(screen.queryByRole('switch', { name: 'Comic Shelf 0' })).toBeNull());
    await user.keyboard('{ArrowDown}');
    expect(tile('Panel Vault 2, For adults (18+)')).toHaveFocus();
    await user.keyboard('{ArrowRight}');
    expect(tile('Panel Vault 6')).toHaveFocus();
  });

  it('draws only the rows near its view of hundreds of sources, and keys reach the rest', async () => {
    const user = userEvent.setup();
    // jsdom doesn't scroll: the grid's box scrolls as far as it's asked.
    let top = 0;
    vi.spyOn(HTMLElement.prototype, 'scrollTop', 'get').mockImplementation(() => top);
    vi.spyOn(HTMLElement.prototype, 'scrollHeight', 'get').mockReturnValue(100_000);
    Object.defineProperty(HTMLElement.prototype, 'scrollTo', {
      configurable: true,
      value(this: HTMLElement, options: ScrollToOptions) {
        top = options.top ?? top;
        this.dispatchEvent(new Event('scroll'));
      },
    });
    panelSize(600);
    localStorage.setItem(
      'raygun.search.customView.manga',
      JSON.stringify({ grouping: 'none', onFirst: false, folded: [] }),
    );
    const grid = await open(sources(400));
    const drawn = tileNames(grid).length;
    expect(drawn).toBeGreaterThan(8);
    expect(drawn).toBeLessThan(120);
    expect(screen.getByText('400 of 400 on')).toBeInTheDocument();

    act(() => tile('Comic Shelf 0').focus());
    await user.keyboard('{Control>}{End}{/Control}');
    await waitFor(() => expect(tile('Manga Scans 399')).toHaveFocus());
    expect(tileNames(grid).length).toBeLessThan(120);
  });

  it('groups many sources by repository, folding groups and switching a whole group', async () => {
    const user = userEvent.setup();
    panelSize(600);
    const grid = await open(sources(30));
    // Many sources come grouped by repository, the ones added by hand last.
    const folds = await within(grid).findAllByRole('button', { expanded: true });
    expect(folds.map((fold) => fold.textContent)).toEqual([
      'Keiyoushi10/10',
      'Paraline10/10',
      'Added by you10/10',
    ]);
    expect(screen.getByRole('combobox', { name: 'Group sources' })).toHaveValue('repository');

    // A group's box switches all of it.
    const paraline = screen.getByRole('checkbox', { name: 'Every source in Paraline' });
    await user.click(paraline);
    expect(paraline).toHaveAttribute('aria-checked', 'false');
    expect(screen.getByText('20 of 30 on')).toBeInTheDocument();
    await user.click(tile('Comic Shelf 0'));
    await user.click(paraline);
    expect(paraline).toHaveAttribute('aria-checked', 'true');
    expect(tile('Comic Shelf 0')).toHaveAttribute('aria-checked', 'true');
    await user.click(tile('Comic Shelf 0'));
    expect(paraline).toHaveAttribute('aria-checked', 'mixed');

    // Folded, a group's tiles go, and All, None and Invert leave them be.
    await user.click(screen.getByRole('button', { name: /^Keiyoushi/ }));
    expect(screen.getByRole('button', { name: /^Keiyoushi/ })).toHaveAttribute(
      'aria-expanded',
      'false',
    );
    expect(screen.queryByRole('switch', { name: 'Daily Strips 1' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'None' }));
    expect(screen.getByText('10 of 30 on')).toBeInTheDocument();
    expect(JSON.parse(localStorage.getItem('raygun.search.customView.manga') ?? '{}')).toEqual({
      grouping: 'repository',
      onFirst: false,
      folded: ['Keiyoushi'],
    });

    // Finding opens every group with a match.
    await user.type(screen.getByRole('searchbox', { name: 'Find a source' }), 'strips 1');
    await waitFor(() => expect(tile('Daily Strips 1')).toBeInTheDocument());

    // No groups: one grid.
    await user.clear(screen.getByRole('searchbox', { name: 'Find a source' }));
    await user.selectOptions(screen.getByRole('combobox', { name: 'Group sources' }), 'No groups');
    expect(within(grid).queryByRole('checkbox')).not.toBeInTheDocument();
    expect(tileNames(grid)).toHaveLength(30);
  });

  it('groups by the languages the catalogue lists', async () => {
    const user = userEvent.setup();
    panelSize(600);
    const listing = (langs: (string | null)[]): RepoCatalog => ({
      repo: { id: 'repo', url: 'https://example.org/index.json', name: 'Repo' },
      error: null,
      entries: langs.map((lang, index) => ({
        repoId: 'repo',
        id: `dev.example.ext${index}`,
        name: `Extension ${index}`,
        version: '1.0.0',
        description: null,
        author: null,
        provides: ['manga.source'],
        net: [],
        contentRating: 'safe',
        lang,
        size: 1,
        state: 'installed',
        nuclear: false,
        installedVersion: '1.0.0',
        icon: null,
      })),
    });
    commands.extensionCatalog.mockResolvedValue(
      ok([
        listing(
          Array.from({ length: 16 }, (_, index) => ['en', 'ja', 'multi', null][index % 4] ?? null),
        ),
      ]),
    );
    const grid = await open(sources(16));
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Group sources' }),
      'By language',
    );
    await waitFor(() =>
      expect(
        within(grid)
          .getAllByRole('button', { expanded: true })
          .map((fold) => fold.textContent),
      ).toEqual(['English4/4', '日本語4/4', 'Many languages4/4', 'Language not listed4/4']),
    );
  });

  it('puts the sources on first when asked, without moving a tile as it is switched', async () => {
    const user = userEvent.setup();
    panelSize(600);
    const grid = await open(sources(16));
    // Few enough not to come grouped.
    expect(screen.getByRole('combobox', { name: 'Group sources' })).toHaveValue('none');
    await user.click(tile('Daily Strips 1'));
    await user.click(tile('Comic Shelf 4'));
    expect(tileNames(grid).slice(0, 2)).toEqual(['Comic Shelf 0', 'Daily Strips 1']);

    await user.click(screen.getByRole('button', { name: 'On first' }));
    expect(screen.getByRole('button', { name: 'On first' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(tileNames(grid).slice(-2)).toEqual(['Daily Strips 1', 'Comic Shelf 4']);
    // Switched now, it stays where it is until the list is arranged again.
    await user.click(tile('Comic Shelf 0'));
    expect(tileNames(grid)[0]).toBe('Comic Shelf 0');
    expect(tile('Comic Shelf 0')).toHaveAttribute('aria-checked', 'false');
  });
});
