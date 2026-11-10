import type { MangaCard, MangaSource } from '@raygun/bindings';
import { cleanup, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const source = (id: string, name: string): MangaSource => ({
  id: `dev.example.${id}/${id}`,
  name,
  extensionId: `dev.example.${id}`,
  enabled: true,
  adult: false,
});
const SHELF = source('shelf', 'Comic Shelf');
const STRIPS = source('strips', 'Daily Strips');
const VAULT = source('vault', 'Panel Vault');
const SOURCES = [SHELF, STRIPS, VAULT];

const card = (id: string, title: string, sourceId: string): MangaCard => ({
  sourceId,
  id,
  title,
  coverUrl: null,
  inLibrary: false,
});

/** The sources asked to search, in the order they were asked. */
const asked = () => commands.catalogGlobalSearch.mock.calls.map((call) => (call[2] as string[])[0]);

const SEARCH = '/browse?tab=search&type=manga&q=dark';

const switches = (panel: HTMLElement) =>
  within(panel)
    .getAllByRole('switch')
    .map((box) => [box.getAttribute('aria-label'), box.getAttribute('aria-checked')]);

describe('Custom sources', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaSources.mockResolvedValue(ok(SOURCES));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, ids) => {
      const found = SOURCES.find((option) => option.id === (ids as string[])[0])!;
      return ok([
        {
          source: found,
          items: [card(`${found.id}-1`, `Dark ${found.name}`, found.id)],
          hasNextPage: false,
          error: null,
        },
      ]);
    });
    commands.catalogFilters.mockResolvedValue(ok([]));
    commands.catalogSearch.mockResolvedValue(
      ok({ items: [card('only', 'Dark Shelf Special', SHELF.id)], hasNextPage: false }),
    );
  });

  it('searches every source while off; on, a switch per source chooses them, remembered', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount(SEARCH);
    const view = await screen.findByTestId('manga-search');
    const pill = await within(view).findByRole('button', { name: /Custom sources/ });
    expect(pill).toHaveAttribute('aria-pressed', 'false');
    await vi.waitFor(() =>
      expect(new Set(asked())).toEqual(new Set(SOURCES.map((option) => option.id))),
    );
    expect(within(view).queryByRole('group', { name: 'Sources' })).not.toBeInTheDocument();

    // On: the panel unfolds under the pill, every source switched on to start with.
    await user.click(pill);
    expect(pill).toHaveAttribute('aria-pressed', 'true');
    const panel = await within(view).findByRole('group', { name: 'Sources' });
    expect(pill).toHaveAttribute('aria-controls', panel.parentElement?.id);
    expect(switches(panel)).toEqual([
      ['Comic Shelf', 'true'],
      ['Daily Strips', 'true'],
      ['Panel Vault', 'true'],
    ]);
    expect(pill).toHaveTextContent('3/3');

    // A source switched off leaves the results.
    const results = await within(view).findByTestId('ranked-results');
    expect(await within(results).findByText('Dark Panel Vault')).toBeInTheDocument();
    await user.click(within(panel).getByRole('switch', { name: 'Panel Vault' }));
    expect(pill).toHaveTextContent('2/3');
    expect(panel).toHaveTextContent('2 of 3 on');
    await vi.waitFor(() =>
      expect(within(view).queryByText('Dark Panel Vault')).not.toBeInTheDocument(),
    );
    expect(within(view).getByText('Dark Comic Shelf')).toBeInTheDocument();
    expect(within(view).getByText('Dark Daily Strips')).toBeInTheDocument();

    // The next time, Custom sources is on with the same sources.
    cleanup();
    await AppShellWrapper.mount(SEARCH);
    const again = await screen.findByTestId('manga-search');
    expect(await within(again).findByRole('button', { name: /Custom sources/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(switches(await within(again).findByRole('group', { name: 'Sources' }))).toEqual([
      ['Comic Shelf', 'true'],
      ['Daily Strips', 'true'],
      ['Panel Vault', 'false'],
    ]);

    // Off again, every source searches; the choice waits for the next time it's on.
    await user.click(within(again).getByRole('button', { name: /Custom sources/ }));
    await vi.waitFor(() =>
      expect(within(again).queryByRole('group', { name: 'Sources' })).not.toBeInTheDocument(),
    );
    expect(await within(again).findByText('Dark Panel Vault')).toBeInTheDocument();
    expect(localStorage.getItem('raygun.search.custom.manga')).toBe('0');
    expect(JSON.parse(localStorage.getItem('raygun.search.sources.manga') ?? '[]')).toEqual([
      SHELF.id,
      STRIPS.id,
    ]);
  });

  it('starts on for sources picked before, searching one alone with its filters', async () => {
    const user = userEvent.setup();
    // Picked with the chips there were before the switch.
    localStorage.setItem('raygun.search.sources.manga', JSON.stringify([SHELF.id]));
    await AppShellWrapper.mount(SEARCH);
    const view = await screen.findByTestId('manga-search');
    expect(await within(view).findByRole('button', { name: /Custom sources/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    const panel = await within(view).findByRole('group', { name: 'Sources' });
    expect(switches(panel)).toEqual([
      ['Comic Shelf', 'true'],
      ['Daily Strips', 'false'],
      ['Panel Vault', 'false'],
    ]);
    expect(await within(view).findByText('Dark Shelf Special')).toBeInTheDocument();
    expect(commands.catalogSearch).toHaveBeenLastCalledWith('manga', SHELF.id, 'dark', 1, {});

    // None leaves nothing to search, and says so; All switches every source back on.
    await user.click(within(panel).getByRole('button', { name: 'None' }));
    expect(await within(view).findByText('Turn on a source above to search it.')).toBeVisible();
    expect(switches(panel).every(([, on]) => on === 'false')).toBe(true);
    await user.click(within(panel).getByRole('button', { name: 'All' }));
    expect(switches(panel).every(([, on]) => on === 'true')).toBe(true);
    expect(await within(view).findByTestId('ranked-results')).toBeInTheDocument();
  });

  it("searches one source alone from its row's More", async () => {
    const user = userEvent.setup();
    localStorage.setItem('raygun.search.grouped.manga', '1');
    await AppShellWrapper.mount(SEARCH);
    const view = await screen.findByTestId('manga-search');
    const row = await within(view).findByRole('region', { name: 'Daily Strips' });
    await user.click(await within(row).findByRole('button', { name: 'More' }));
    const panel = await within(view).findByRole('group', { name: 'Sources' });
    expect(switches(panel)).toEqual([
      ['Comic Shelf', 'false'],
      ['Daily Strips', 'true'],
      ['Panel Vault', 'false'],
    ]);
    expect(commands.catalogSearch).toHaveBeenLastCalledWith('manga', STRIPS.id, 'dark', 1, {});
  });
});
