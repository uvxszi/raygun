import type { Captions, Lyrics, QueueItem, TrackQuery } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { FC } from 'react';

import { LyricsView } from '../player/LyricsView';
import { bindingsMock, emitEvent, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { TranscriptionStatus } from './TranscriptionStatus';
import {
  heardPercent,
  songCaptionsKey,
  transcriptionStage,
  useSongTranscription,
  useTranscribedLyricsSync,
} from './useSongTranscription';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SONG: TrackQuery = {
  title: 'Ghostbusters (Remastered)',
  artist: 'Ray Parker Jr.',
  album: null,
  durationMs: 240_000,
};
const KEY = songCaptionsKey(SONG);

const captions = (overrides: Partial<Captions> = {}): Captions => ({
  episodeId: KEY,
  cues: [],
  source: null,
  covered: [],
  complete: false,
  running: false,
  preparing: false,
  error: null,
  canTranscribe: true,
  engine: 'local',
  hasFeed: false,
  queued: null,
  durationMs: null,
  ...overrides,
});

const Harness: FC = () => {
  const transcription = useSongTranscription(SONG);
  useTranscribedLyricsSync();
  return (
    <div>
      <button type="button" disabled={transcription.busy} onClick={transcription.start}>
        Hear
      </button>
      <TranscriptionStatus transcription={transcription} />
    </div>
  );
};

beforeEach(() => {
  seedBindings();
});

describe('song transcription', () => {
  it('names the key as Rust does and reads the stage and progress', () => {
    expect(KEY).toBe('song:ray parker jr|ghostbusters');
    expect(transcriptionStage(undefined)).toBe('idle');
    expect(transcriptionStage(captions({ queued: 2 }))).toBe('queued');
    expect(transcriptionStage(captions({ running: true, preparing: true }))).toBe('preparing');
    expect(transcriptionStage(captions({ running: true }))).toBe('hearing');
    expect(heardPercent(captions({ covered: [[0, 60_000]], durationMs: 240_000 }))).toBe(25);
    expect(heardPercent(captions({ covered: [[0, 60_000]] }))).toBeNull();
  });

  it('shows the place in line, then how far it heard, and stops', async () => {
    let state = captions();
    commands.captions.mockImplementation(async () => ok(state));
    commands.lyricsTranscribe.mockImplementation(async () => {
      state = captions({ queued: 2 });
      return ok(KEY);
    });
    const user = userEvent.setup();
    renderWithProviders(<Harness />);

    const hear = await screen.findByRole('button', { name: 'Hear' });
    await user.click(hear);
    expect(await screen.findByRole('status')).toHaveTextContent('2 in line');
    expect(hear).toBeDisabled();

    state = captions({ running: true, covered: [[0, 120_000]], durationMs: 240_000 });
    emitEvent('captionsEvent', KEY);
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('50 %'));

    await user.click(screen.getByRole('button', { name: 'Stop' }));
    expect(commands.captionsStop).toHaveBeenCalledWith(KEY);
  });

  it('names a failure and offers to try again', async () => {
    commands.captions.mockResolvedValue(
      ok(
        captions({
          error:
            "Couldn't read YouTube's stream of this song\nYouTube's stream: YouTube didn't give a stream for it",
        }),
      ),
    );
    renderWithProviders(<Harness />);
    expect(await screen.findByText(/Couldn't read YouTube's stream/)).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole('button', { name: /try again/i }));
    expect(commands.lyricsTranscribe).toHaveBeenCalledWith(SONG);
  });

  it('refreshes lyrics when a song was heard to the end', async () => {
    commands.captions.mockResolvedValue(ok(captions()));
    renderWithProviders(<Harness />);
    await screen.findByRole('button', { name: 'Hear' });
    commands.captions.mockClear();
    emitEvent('lyricsTranscribedEvent', KEY);
    await waitFor(() => expect(commands.captions).toHaveBeenCalledWith(KEY));
  });
});

describe('transcribed lyrics in the player', () => {
  const ITEM: QueueItem = {
    id: 'q1',
    title: SONG.title,
    artist: SONG.artist,
    album: null,
    durationMs: 240_000,
    artworkUrl: null,
    mediaKind: 'audio',
    source: { kind: 'url', url: 'https://example.com/ghost.flac', headers: {} },
  };
  const HEARD: Lyrics = {
    providerName: 'Your lyrics',
    synced: [{ timeMs: 1_000, text: 'Who you gonna call' }],
    plain: null,
    offsetMs: 0,
    edited: true,
    translations: [],
  };
  const SOURCE: Lyrics = {
    ...HEARD,
    providerName: 'Lyric Lines',
    synced: [{ timeMs: 1_000, text: "If there's something strange" }],
    edited: false,
  };

  it('shows the lines heard so far while the song is being heard', async () => {
    commands.lyricsGet.mockResolvedValue(ok(SOURCE));
    commands.captions.mockResolvedValue(
      ok(
        captions({
          running: true,
          cues: [
            { startMs: 31_000, endMs: 33_000, text: 'Ghostbusters' },
            { startMs: 1_000, endMs: 3_000, text: 'Who you gonna call' },
          ],
        }),
      ),
    );
    renderWithProviders(<LyricsView item={ITEM} />);
    const view = await screen.findByTestId('lyrics-view');
    const lines = await within(view).findAllByRole('button', {
      name: /Who you gonna call|Ghostbusters/,
    });
    expect(lines.map((line) => line.textContent)).toEqual(['Who you gonna call', 'Ghostbusters']);
  });

  it("deletes saved transcribed lyrics and shows a source's instead", async () => {
    const user = userEvent.setup();
    let deleted = false;
    commands.lyricsGet.mockImplementation(async () => ok(deleted ? SOURCE : HEARD));
    commands.lyricsForget.mockImplementation(async () => {
      deleted = true;
      return ok(null);
    });
    renderWithProviders(<LyricsView item={ITEM} />);
    const view = await screen.findByTestId('lyrics-view');
    await within(view).findByRole('button', { name: /Who you gonna call/ });
    await user.click(within(view).getByRole('button', { name: /Lyrics from/ }));
    const menu = await screen.findByTestId('lyrics-source-menu');
    await user.click(within(menu).getByRole('button', { name: 'Delete saved lyrics' }));
    expect(commands.lyricsForget).toHaveBeenCalledWith(
      expect.objectContaining({ title: SONG.title, artist: SONG.artist }),
    );
    expect(await within(view).findByRole('button', { name: /something strange/ })).toBeVisible();
    expect(within(view).queryByRole('button', { name: /Who you gonna call/ })).toBeNull();
  });

  it('says why deleting failed, keeping the menu open', async () => {
    const user = userEvent.setup();
    commands.lyricsGet.mockResolvedValue(ok(HEARD));
    commands.lyricsForget.mockResolvedValue({ status: 'error', error: 'Disk is full' });
    renderWithProviders(<LyricsView item={ITEM} />);
    const view = await screen.findByTestId('lyrics-view');
    await user.click(await within(view).findByRole('button', { name: /Lyrics from/ }));
    const menu = await screen.findByTestId('lyrics-source-menu');
    await user.click(within(menu).getByRole('button', { name: 'Delete saved lyrics' }));
    await waitFor(() => expect(menu).toHaveTextContent('Disk is full'));
  });
});
