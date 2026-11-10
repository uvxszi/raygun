import type { OnlineTrack, TrackInput } from '@raygun/bindings';
import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { differences } from './sourcePick';
import { useSourcePickerStore } from './sourcePickerStore';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

// Imported from a screenshot with its names misread.
const SONG: TrackInput = {
  title: 'Sogn',
  artist: 'Artst',
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
};

const PICK: OnlineTrack = {
  externalId: 'd1',
  title: 'Song',
  artist: 'Artist',
  album: 'Album',
  durationMs: 200_000,
  artworkUrl: 'https://img.example/a.jpg',
  source: null,
  explicit: false,
};

const PROVIDER = {
  id: 'dev.example.tunes/search',
  extensionId: 'dev.example.tunes',
  enabled: true,
  kind: 'music.metadata',
  name: 'Tunes',
};

describe('Choosing a source', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [{ provider: PROVIDER, tracks: [PICK], error: null }],
      }),
    );
  });

  it('lists the details that differ', () => {
    expect(differences(SONG, PICK).map((difference) => difference.field)).toEqual([
      'title',
      'artist',
      'album',
      'cover',
    ]);
    expect(differences({ ...SONG, title: 'Song', artist: 'Artist', album: 'Album' }, PICK)).toEqual(
      [],
    );
  });

  it("offers the source's details for an imported song and assigns them with its stream", async () => {
    const user = userEvent.setup();
    commands.trackImportStatus.mockResolvedValue(ok({ imported: true, unavailable: true }));
    await AppShellWrapper.mount('/library?view=playlists');
    act(() => useSourcePickerStore.getState().open(SONG));
    const picker = await screen.findByTestId('source-picker');
    await user.click(await within(picker).findByRole('button', { name: 'Use this' }));

    // Side by side, the switch on for an imported song; the cover left out.
    const confirm = await within(picker).findByTestId('source-pick-confirm');
    expect(confirm).toHaveTextContent('Sogn');
    expect(confirm).toHaveTextContent('Song');
    await vi.waitFor(() =>
      expect(within(confirm).getByRole('switch', { name: 'Use these details' })).toHaveAttribute(
        'aria-checked',
        'true',
      ),
    );
    await user.click(within(confirm).getByRole('checkbox', { name: 'Cover' }));
    await user.click(within(confirm).getByRole('button', { name: 'Use this source' }));
    expect(commands.trackUseSource).toHaveBeenCalledWith(SONG, PICK, {
      assign: true,
      sourceName: 'Tunes',
      title: true,
      artist: true,
      album: true,
      cover: false,
    });
    expect(commands.trackAssignSource).not.toHaveBeenCalled();
  });

  it('takes the details of the source Raygun found by itself, keeping its stream', async () => {
    const user = userEvent.setup();
    const found: OnlineTrack = {
      externalId: 'ia:7',
      title: 'Song',
      artist: 'Artist',
      album: 'Album',
      durationMs: 200_000,
      artworkUrl: 'https://img.example/found.jpg',
      source: null,
      explicit: false,
    };
    commands.trackFoundSource.mockResolvedValue(
      ok({
        providerId: 'dev.example.archive/stream',
        providerName: 'Archive',
        track: found,
        points: 0,
      }),
    );
    await AppShellWrapper.mount('/library?view=playlists');
    act(() => useSourcePickerStore.getState().open(SONG));
    const picker = await screen.findByTestId('source-picker');
    const card = await within(picker).findByTestId('found-source');
    expect(commands.trackFoundSource).toHaveBeenCalledWith(SONG);
    expect(card).toHaveTextContent('Raygun plays it from Archive');
    expect(card).toHaveTextContent('Artist · Album');
    await user.click(within(card).getByRole('button', { name: 'Use details from Archive' }));

    // Every detail that differs is ticked, with no switch: details are all it takes.
    const confirm = await within(picker).findByTestId('source-pick-confirm');
    expect(within(confirm).queryByRole('switch')).not.toBeInTheDocument();
    expect(within(confirm).getByRole('checkbox', { name: 'Title' })).toBeChecked();
    await user.click(within(confirm).getByRole('checkbox', { name: 'Album' }));
    await user.click(within(confirm).getByRole('button', { name: 'Use these details' }));
    expect(commands.trackUseSource).toHaveBeenCalledWith(SONG, found, {
      assign: false,
      sourceName: 'Archive',
      title: true,
      artist: true,
      album: false,
      cover: true,
    });
    // Its names were taken, not typed: nothing more is renamed.
    expect(commands.detailEditSet).not.toHaveBeenCalled();
    expect(
      await within(picker).findByText('The song shows the details from Archive now.'),
    ).toBeInTheDocument();
  });

  it('shows no found source until one played the song', async () => {
    await AppShellWrapper.mount('/library?view=playlists');
    act(() => useSourcePickerStore.getState().open(SONG));
    const picker = await screen.findByTestId('source-picker');
    await within(picker).findByRole('button', { name: 'Use this' });
    expect(within(picker).queryByTestId('found-source')).not.toBeInTheDocument();
  });

  it('keeps the names of a song that came from elsewhere unless asked', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?view=playlists');
    act(() => useSourcePickerStore.getState().open(SONG));
    const picker = await screen.findByTestId('source-picker');
    await user.click(await within(picker).findByRole('button', { name: 'Use this' }));
    const confirm = await within(picker).findByTestId('source-pick-confirm');
    expect(within(confirm).getByRole('switch', { name: 'Use these details' })).toHaveAttribute(
      'aria-checked',
      'false',
    );
    await user.click(within(confirm).getByRole('button', { name: 'Use this source' }));
    expect(commands.trackUseSource).toHaveBeenCalledWith(
      SONG,
      PICK,
      expect.objectContaining({ assign: true, title: false, artist: false, album: false }),
    );
  });
});
