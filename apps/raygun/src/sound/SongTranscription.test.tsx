import type { Captions, TrackQuery } from '@raygun/bindings';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { FC } from 'react';

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
            'Only YouTube has this song, and its player gives no sound to hear: download the song first',
        }),
      ),
    );
    renderWithProviders(<Harness />);
    expect(await screen.findByText(/Only YouTube has this song/)).toBeInTheDocument();
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
