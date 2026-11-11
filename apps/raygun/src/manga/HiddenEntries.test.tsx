import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const hidden = (titleId: string, title: string) => ({
  titleId,
  sourceId: 'dev.example/comics',
  remoteId: titleId,
  title,
  sourceName: 'Example Comics',
  coverUrl: null,
});

describe('Hidden library entries', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaLibrary.mockResolvedValue(
      ok([libraryEntry({ titleId: 'a', remoteId: 'a', title: 'Frieren' })]),
    );
  });

  it('says how many are hidden and lists them with the way to the setting', async () => {
    const user = userEvent.setup();
    commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, showNsfw: false }));
    commands.libraryHidden.mockResolvedValue(
      ok([hidden('h1', 'Night Tales'), hidden('h2', 'Late Shift'), hidden('h3', 'After Dark')]),
    );
    await AppShellWrapper.mount('/library?media=manga');
    await user.click(await screen.findByRole('button', { name: '3 hidden entries' }));
    expect(commands.libraryHidden).toHaveBeenCalledWith('manga');

    const sheet = await screen.findByRole('dialog', { name: 'Hidden entries' });
    expect(within(sheet).getByText(/18\+ content is turned off in Settings/)).toBeInTheDocument();
    const list = within(sheet).getByRole('list', { name: 'Hidden entries' });
    expect(
      within(list)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual([
      'Night TalesExample Comics',
      'Late ShiftExample Comics',
      'After DarkExample Comics',
    ]);
    expect(within(sheet).getByRole('link', { name: /18\+ content in Settings/ })).toHaveAttribute(
      'href',
      '/settings/browse',
    );
  });

  it('says nothing while 18+ content is on', async () => {
    commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, showNsfw: true }));
    commands.libraryHidden.mockResolvedValue(ok([hidden('h1', 'Night Tales')]));
    await AppShellWrapper.mount('/library?media=manga');
    expect(await screen.findByText('Frieren')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /hidden entr/ })).not.toBeInTheDocument();
    expect(commands.libraryHidden).not.toHaveBeenCalled();
  });
});
