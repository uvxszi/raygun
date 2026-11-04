import type { MangaUpdateStatus, SourceFilter } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { IDLE_UPDATE, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.raygun.mangadex/mangadex';

const card = (id: string, title: string) => ({
  sourceId: SOURCE,
  id,
  title,
  coverUrl: null,
  inLibrary: false,
});

const FILTERS: SourceFilter[] = [
  {
    kind: 'select',
    key: 'status',
    label: 'Status',
    default: 'any',
    options: [
      { value: 'any', label: 'Any' },
      { value: 'completed', label: 'Completed' },
    ],
  },
];

describe('A source’s page and library updates', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    sessionStorage.clear();
    commands.mangaSources.mockResolvedValue(
      ok([{ id: SOURCE, name: 'MangaDex', extensionId: 'dev.raygun.mangadex', enabled: true }]),
    );
    commands.mangaBrowse.mockResolvedValue(
      ok({ items: [card('p1', 'Popular one')], hasNextPage: false }),
    );
  });

  it('browses a source with its own filters', async () => {
    const user = userEvent.setup();
    commands.catalogFilters.mockResolvedValue(ok(FILTERS));
    commands.catalogSearch.mockResolvedValue(
      ok({ items: [card('f1', 'Finished one')], hasNextPage: false }),
    );
    await AppShellWrapper.mount(`/manga/source?id=${encodeURIComponent(SOURCE)}`);
    const page = await screen.findByTestId('manga-source-page');
    expect(await within(page).findByText('Popular one')).toBeInTheDocument();

    await user.click(await within(page).findByRole('button', { name: 'Filters' }));
    await user.selectOptions(within(page).getByRole('combobox', { name: 'Status' }), 'completed');
    expect(await within(page).findByText('Finished one')).toBeInTheDocument();
    expect(commands.catalogSearch).toHaveBeenLastCalledWith('manga', SOURCE, '', 1, {
      status: 'completed',
    });
    expect(within(page).getByRole('button', { name: '1 filter' })).toBeInTheDocument();

    // A list again drops the filters.
    await user.click(within(page).getByRole('button', { name: 'Popular' }));
    expect(await within(page).findByText('Popular one')).toBeInTheDocument();
    expect(within(page).getByRole('button', { name: 'Filters' })).toBeInTheDocument();
  });

  it('names the source when it fails, and asks it again', async () => {
    const user = userEvent.setup();
    commands.mangaBrowse.mockResolvedValueOnce({ status: 'error', error: 'HTTP 503' });
    await AppShellWrapper.mount(`/manga/source?id=${encodeURIComponent(SOURCE)}`);
    const page = await screen.findByTestId('manga-source-page');
    const issue = await within(page).findByTestId('source-issue');
    // A server error reads as the source having problems (lib/issue.ts).
    expect(issue).toHaveTextContent('MangaDex is having problems');
    await user.click(within(issue).getByRole('button', { name: 'Try again' }));
    expect(await within(page).findByText('Popular one')).toBeInTheDocument();
  });

  it('lists the titles an update couldn’t check, with their sources', async () => {
    const user = userEvent.setup();
    const status: MangaUpdateStatus = {
      ...IDLE_UPDATE,
      checked: 2,
      total: 2,
      failed: 1,
      lastRunAt: 1_700_000_000_000,
      failures: [
        {
          medium: 'novel',
          sourceId: 'dev.raygun.gutenberg/gutenberg',
          remoteId: '84',
          title: 'Frankenstein',
          sourceName: 'Project Gutenberg',
          message: 'HTTP 500',
        },
      ],
    };
    commands.mangaUpdateStatus.mockResolvedValue(ok(status));
    commands.mangaUpdates.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/updates?tab=chapters');
    const page = await screen.findByTestId('updates-page');
    expect(await within(page).findByText(/1 title failed/)).toBeInTheDocument();
    await user.click(within(page).getByRole('button', { name: 'Show the titles that failed' }));
    const failures = within(page).getByTestId('update-failures');
    expect(
      within(failures).getByRole('link', { name: 'Frankenstein' }).getAttribute('href'),
    ).toMatch(/^\/novel\/title\?source=dev\.raygun\.gutenberg%2Fgutenberg&id=/);
    expect(within(failures).getByTestId('source-issue')).toHaveTextContent(
      'Project Gutenberg is having problems',
    );
  });
});
