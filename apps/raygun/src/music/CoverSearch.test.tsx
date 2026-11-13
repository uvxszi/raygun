import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { useSongEditStore } from '../sound/songEditStore';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

// Imported with its title misspelt: no source has a cover under it.
const SONG = { title: 'Feelin Gud', artist: 'Nina Simone', album: null, durationMs: null };

describe('Covers from the sources', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.detailEdit.mockResolvedValue(
      ok({ title: null, artist: null, album: null, artworkUrl: null }),
    );
  });

  it('searches by the names shown above the covers, and again by names typed there', async () => {
    const user = userEvent.setup();
    commands.detailCoverOptions.mockImplementation(async (target) =>
      ok(
        target.kind === 'song' && target.query.title === 'Feeling Good'
          ? [{ url: 'https://img.example/fg.jpg', source: 'Tunes', of: 'I Put a Spell on You' }]
          : [],
      ),
    );
    await AppShellWrapper.mount('/');
    act(() => useSongEditStore.getState().open(SONG, { tab: 'details' }));
    const form = await screen.findByTestId('detail-edit');

    await user.click(within(form).getByRole('button', { name: 'Find covers in your sources' }));
    expect(commands.detailCoverOptions).toHaveBeenLastCalledWith(
      { kind: 'song', query: SONG },
      expect.any(String),
    );
    expect(
      await within(form).findByText('Your sources had no other covers for this.'),
    ).toBeVisible();

    // The names it searched by, ready to fix.
    const names = within(form).getByRole('search', { name: 'Names to find covers by' });
    const title = within(names).getByRole('textbox', { name: 'Song title to search for' });
    expect(title).toHaveValue('Feelin Gud');
    expect(within(names).getByRole('textbox', { name: 'Artist to search for' })).toHaveValue(
      'Nina Simone',
    );
    await user.clear(title);
    // Enter searches; it doesn't save the details around it.
    await user.type(title, 'Feeling Good{Enter}');
    expect(commands.detailCoverOptions).toHaveBeenLastCalledWith(
      { kind: 'song', query: { ...SONG, title: 'Feeling Good' } },
      expect.any(String),
    );
    expect(commands.detailEditSet).not.toHaveBeenCalled();
    const covers = await within(form).findByTestId('source-covers');
    await user.click(
      within(covers).getByRole('button', {
        name: 'Use the cover from Tunes · I Put a Spell on You',
      }),
    );
    await user.click(within(form).getByRole('button', { name: 'Save' }));
    // Only the cover changes: the song keeps its names.
    expect(commands.detailEditSet).toHaveBeenCalledWith(
      { kind: 'song', query: SONG },
      { title: null, artist: null, album: null, artworkUrl: 'https://img.example/fg.jpg' },
    );
  });

  it('shows covers as each source answers, before the slowest one', async () => {
    const user = userEvent.setup();
    let finish: (covers: { url: string; source: string; of: string | null }[]) => void = () => {};
    commands.detailCoverOptions.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = (covers) => resolve(ok(covers));
        }),
    );
    await AppShellWrapper.mount('/');
    act(() => useSongEditStore.getState().open(SONG, { tab: 'details' }));
    const form = await screen.findByTestId('detail-edit');
    await user.click(within(form).getByRole('button', { name: 'Find covers in your sources' }));
    const request = commands.detailCoverOptions.mock.lastCall?.[1] ?? '';
    const fast = { url: 'https://img.example/fast.jpg', source: 'Fast', of: null };
    // Another search's covers aren't these.
    emitEvent('coverOptionsEvent', ['other', [fast]]);
    expect(within(form).queryByTestId('source-covers')).toBeNull();
    emitEvent('coverOptionsEvent', [request, [fast]]);
    const covers = await within(form).findByTestId('source-covers');
    expect(within(covers).getByRole('button', { name: 'Use the cover from Fast' })).toBeVisible();
    // Still looking: the slower sources haven't answered.
    expect(within(form).getByText('Looking in your sources…')).toBeVisible();
    const slow = { url: 'https://img.example/slow.jpg', source: 'Slow', of: null };
    await act(async () => finish([fast, slow]));
    expect(
      await within(form).findByRole('button', { name: 'Use the cover from Slow' }),
    ).toBeVisible();
    expect(within(form).queryByText('Looking in your sources…')).toBeNull();
  });
});
