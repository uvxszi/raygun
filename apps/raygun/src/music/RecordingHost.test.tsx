import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { RecordingHost } from './RecordingHost';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const EMBED = 'http://localhost:4000/embed/youtube';

const request = (videoId: string | null) => ({
  session: 's1',
  downloadId: 'd1',
  title: 'Ballad',
  artist: 'Band',
  videoId,
  startMs: 0,
  embedUrl: EMBED,
});

/** What the embed page in the recording's player says. */
const reply = (frame: HTMLIFrameElement, data: object) =>
  act(() => {
    window.dispatchEvent(
      new MessageEvent('message', {
        data: { raygunEmbed: true, ...data },
        origin: 'http://localhost:4000',
        source: frame.contentWindow,
      }),
    );
  });

describe('RecordingHost', () => {
  it('explains recording and asks to allow it, or not now', async () => {
    const user = userEvent.setup();
    renderWithProviders(<RecordingHost />);

    emitEvent('recordingConsentEvent', { songs: 2 });
    const ask = await screen.findByTestId('recording-consent');
    expect(ask).toHaveTextContent('2 songs can only be saved by recording them as they play');
    expect(ask).toHaveTextContent('turning the volume down or muting doesn’t affect');
    await user.click(within(ask).getByRole('button', { name: 'Allow recording' }));
    expect(commands.songRecordingConsent).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('recording-consent')).not.toBeInTheDocument();

    emitEvent('recordingConsentEvent', { songs: 1 });
    const again = await screen.findByTestId('recording-consent');
    expect(again).toHaveTextContent('1 song can only be saved by recording it as it plays');
    await user.click(within(again).getByRole('button', { name: 'Not now' }));
    expect(commands.songRecordingDecline).toHaveBeenCalledTimes(1);
    expect(commands.songRecordingConsent).toHaveBeenCalledTimes(1);
  });

  it('plays the song aloud in a player of its own, reports it, and can stop it', async () => {
    const user = userEvent.setup();
    renderWithProviders(<RecordingHost />);

    emitEvent('songRecordingEvent', request('abc'));
    const frame = document.querySelector<HTMLIFrameElement>('iframe[title="Recording player"]');
    expect(frame?.src).toBe(EMBED);
    const card = await screen.findByTestId('recording-card');
    expect(card).toHaveTextContent('Recording Ballad');

    // The page is ready: it's asked for the video at full volume, and what it does goes to Rust.
    const posted = vi.spyOn(frame!.contentWindow!, 'postMessage');
    reply(frame!, { type: 'ready' });
    expect(posted).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'load', videoId: 'abc', autoplay: true, volume: 1 }),
      'http://localhost:4000',
    );
    reply(frame!, { type: 'state', state: 'playing', token: 1 });
    reply(frame!, { type: 'progress', positionMs: 60_000, durationMs: 240_000, token: 1 });
    expect(commands.songRecordingReport).toHaveBeenCalledWith({
      session: 's1',
      event: { type: 'playing' },
    });
    expect(commands.songRecordingReport).toHaveBeenCalledWith({
      session: 's1',
      event: { type: 'progress', positionMs: 60_000, durationMs: 240_000 },
    });
    expect(within(card).getByRole('progressbar')).toHaveAttribute('aria-valuenow', '25');

    await user.click(within(card).getByRole('button', { name: 'Stop recording Ballad' }));
    expect(commands.songDownloadCancelRecording).toHaveBeenCalledWith('d1');

    // Rust ends the run: the player stops and the card goes.
    emitEvent('songRecordingEvent', request(null));
    expect(screen.queryByTestId('recording-card')).not.toBeInTheDocument();
    expect(posted).toHaveBeenLastCalledWith(
      expect.objectContaining({ type: 'stop' }),
      'http://localhost:4000',
    );
  });
});
