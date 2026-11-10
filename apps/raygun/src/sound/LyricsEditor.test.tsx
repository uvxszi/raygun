import type { Lyrics, QueueItem, SongEdit } from '@raygun/bindings';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { currentLine } from '../player/lyricsQuery';
import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { IDLE_STATE, bindingsMock } from '../test/bindingsMock';
import { formatOffset, parseOffset } from './format';
import { useSongEditStore } from './songEditStore';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SONG = { title: 'SWIM', artist: 'BTS', album: null, durationMs: null };
const SOURCE: Lyrics = {
  providerName: 'Lyrics Base',
  synced: [
    { timeMs: 10_000, text: 'Swim, swim' },
    { timeMs: 14_000, text: 'Water falling off your skin' },
  ],
  plain: null,
  offsetMs: 0,
  edited: false,
};
const ITEM: QueueItem = {
  id: 'q1',
  title: 'SWIM',
  artist: 'BTS',
  album: null,
  durationMs: 157_000,
  artworkUrl: null,
  mediaKind: 'audio',
  source: { kind: 'url', url: 'https://example.com/swim.flac', headers: {} },
};

/** Song edits that Rust stores and lyrics that follow them, as the real commands do. */
const withSong = (lyrics: Lyrics | null) => {
  let stored: SongEdit = {};
  commands.songEditGet.mockImplementation(async () => stored);
  commands.songEditSet.mockImplementation(async (_query, edit: SongEdit) => {
    stored = edit;
    return ok(edit);
  });
  commands.lyricsGet.mockImplementation(async () => {
    const own = stored.lyrics;
    if (own) {
      return ok({
        ...own,
        providerName: 'Your lyrics',
        offsetMs: stored.lyricsOffsetMs ?? 0,
        edited: true,
      } as Lyrics);
    }
    return ok(lyrics && { ...lyrics, offsetMs: stored.lyricsOffsetMs ?? 0 });
  });
};

const lastEdit = () => commands.songEditSet.mock.calls.at(-1)?.[1] as SongEdit | undefined;

const openLyrics = async () => {
  await AppShellWrapper.mount('/');
  useSongEditStore.getState().open(SONG, { tab: 'lyrics' });
  return screen.findByTestId('lyrics-editor');
};

describe('The lyrics editor', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('finds the line playing with an offset', () => {
    const lines = SOURCE.synced;
    expect(currentLine(lines, 12_000, 0)).toBe(0);
    // Shown 3 s later: at 12 s nothing has shown yet; at 17 s the second line has.
    expect(currentLine(lines, 12_000, 3_000)).toBe(-1);
    expect(currentLine(lines, 17_000, 3_000)).toBe(1);
  });

  it('shifts the lyrics, and syncs them to a line as it is heard', async () => {
    const user = userEvent.setup();
    withSong(SOURCE);
    commands.playbackGetState.mockResolvedValue({
      ...IDLE_STATE,
      status: 'paused',
      currentItemId: 'q1',
      positionMs: 15_500,
    });
    commands.playbackGetQueue.mockResolvedValue({ items: [ITEM], order: [0], context: null });
    const editor = await openLyrics();

    await user.click(await within(editor).findByRole('button', { name: 'Show the lines later' }));
    await waitFor(() => expect(lastEdit()?.lyricsOffsetMs).toBe(100));
    await user.click(within(editor).getByRole('button', { name: 'Show the lines later' }));
    await waitFor(() =>
      expect(within(editor).getByRole('textbox', { name: 'Timing in seconds' })).toHaveValue(
        '+0.2',
      ),
    );

    // "Water falling…" is sung now, at 15.5 s: its line (14 s) moves 1.5 s later.
    await user.click(
      within(editor).getByRole('button', { name: 'Sync to “Water falling off your skin” now' }),
    );
    await waitFor(() => expect(lastEdit()?.lyricsOffsetMs).toBe(1_500));
  });

  it('reads typed offsets in seconds or milliseconds', () => {
    expect(parseOffset('1.25')).toBe(1_250);
    expect(parseOffset('-0.4 s')).toBe(-400);
    expect(parseOffset('−1,5')).toBe(-1_500);
    expect(parseOffset('+2')).toBe(2_000);
    expect(parseOffset('.5')).toBe(500);
    expect(parseOffset('250 ms')).toBe(250);
    expect(parseOffset('9999')).toBe(600_000);
    expect(parseOffset('')).toBeNull();
    expect(parseOffset('-')).toBeNull();
    expect(parseOffset('soon')).toBeNull();
    expect([0, 200, -1_250, 1_000].map(formatOffset)).toEqual(['0.0', '+0.2', '-1.25', '+1.0']);
  });

  it('takes a typed offset on Enter or when the field is left, keeping − and +', async () => {
    const user = userEvent.setup();
    withSong(SOURCE);
    const editor = await openLyrics();
    const field = await within(editor).findByRole('textbox', { name: 'Timing in seconds' });
    expect(field).toHaveValue('0.0');

    await user.clear(field);
    await user.type(field, '1.25{Enter}');
    await waitFor(() => expect(lastEdit()?.lyricsOffsetMs).toBe(1_250));
    await waitFor(() => expect(field).toHaveValue('+1.25'));

    // Left without Enter: taken too.
    await user.clear(field);
    await user.type(field, '-0.5');
    await user.tab();
    await waitFor(() => expect(lastEdit()?.lyricsOffsetMs).toBe(-500));

    // Not a time: nothing changes, and the field shows the offset again.
    const saves = commands.songEditSet.mock.calls.length;
    await user.clear(field);
    await user.type(field, 'soon');
    expect(field).toHaveAttribute('aria-invalid', 'true');
    await user.keyboard('{Enter}');
    expect(field).toHaveValue('-0.5');
    expect(commands.songEditSet).toHaveBeenCalledTimes(saves);

    // The buttons stay, and step from the typed offset.
    await user.click(within(editor).getByRole('button', { name: 'Show the lines later' }));
    await waitFor(() => expect(lastEdit()?.lyricsOffsetMs).toBe(-400));
  });

  it('edits lines and their times, stamping the next line while the song plays', async () => {
    const user = userEvent.setup();
    withSong(SOURCE);
    commands.playbackGetState.mockResolvedValue({
      ...IDLE_STATE,
      status: 'paused',
      currentItemId: 'q1',
      positionMs: 9_200,
    });
    commands.playbackGetQueue.mockResolvedValue({ items: [ITEM], order: [0], context: null });
    const editor = await openLyrics();

    await user.click(await within(editor).findByRole('button', { name: 'Edit lines and times' }));
    await user.click(within(editor).getByRole('button', { name: 'Stamp: “Swim, swim”' }));
    expect(within(editor).getByRole('textbox', { name: 'Line 1 time' })).toHaveValue('0:09.2');

    const words = within(editor).getByRole('textbox', { name: 'Line 2 words' });
    await user.clear(words);
    await user.type(words, 'Water falling off your skin, yeah');
    await user.click(within(editor).getByRole('button', { name: 'Add a line' }));
    fireEvent.change(within(editor).getByRole('textbox', { name: 'Line 3 words' }), {
      target: { value: 'Swim' },
    });
    await user.click(within(editor).getByRole('button', { name: 'Save lyrics' }));

    await waitFor(() =>
      expect(lastEdit()?.lyrics?.synced).toEqual([
        { timeMs: 9_200, text: 'Swim, swim' },
        { timeMs: 14_000, text: 'Water falling off your skin, yeah' },
        { timeMs: 15_000, text: 'Swim' },
      ]),
    );
    // Back to what the source has.
    await user.click(
      await within(editor).findByRole('button', { name: /Use the source’s lyrics/ }),
    );
    await waitFor(() => expect(lastEdit()?.lyrics).toBeNull());
  });

  it('writes lyrics no source has, then times them line by line', async () => {
    const user = userEvent.setup();
    withSong(null);
    const editor = await openLyrics();

    const box = await within(editor).findByRole('textbox', { name: 'Lyrics' });
    await user.type(box, 'First line{Enter}{Enter}Second line');
    await user.click(within(editor).getByRole('button', { name: 'Time them line by line' }));
    const second = within(editor).getByRole('textbox', { name: 'Line 2 time' });
    await user.clear(second);
    await user.type(second, '0:21.5');
    await user.click(within(editor).getByRole('button', { name: 'Save lyrics' }));

    await waitFor(() =>
      expect(lastEdit()?.lyrics).toEqual({
        synced: [
          { timeMs: 0, text: 'First line' },
          { timeMs: 21_500, text: 'Second line' },
        ],
        plain: null,
      }),
    );
  });
});
