import type { SongCaptureRequest } from '@raygun/bindings';
import { act, render } from '@testing-library/react';
import type { FC } from 'react';

import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { toBase64 } from './songCapture';
import { useYouTubeCaptureHost } from './youtubeCapture';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const HOST = 'http://localhost:4000';

const request = (extra: Partial<SongCaptureRequest> = {}): SongCaptureRequest => ({
  session: 's1',
  url: null,
  format: null,
  fromMs: 1_500,
  speed: 1,
  videoId: 'abc123',
  embedUrl: `${HOST}/embed/youtube`,
  tapMode: 'muted',
  ...extra,
});

const Host: FC = () => {
  useYouTubeCaptureHost();
  return null;
};

const playerFrame = () =>
  document.querySelector<HTMLIFrameElement>('iframe[title="Capture player"]');

/** The hidden player's frame, with YouTube's frame inside it (as the IFrame API makes one). */
const withYouTubeFrame = (frame: HTMLIFrameElement) => {
  const page = frame.contentDocument!;
  const root = page.documentElement ?? page.appendChild(page.createElement('html'));
  const body = page.body ?? root.appendChild(page.createElement('body'));
  const inner = page.createElement('iframe');
  body.append(inner);
  return inner.contentWindow!;
};

const post = (data: object, origin: string, source: Window | null) =>
  act(() => {
    window.dispatchEvent(new MessageEvent('message', { data, origin, source }));
  });

describe('useYouTubeCaptureHost', () => {
  it('plays the video in a hidden player of its own and reports what the tap hears', async () => {
    const { unmount } = render(<Host />);
    act(() => emitEvent('songCaptureEvent', request()));
    const frame = playerFrame()!;
    expect(frame.src).toBe(`${HOST}/embed/youtube`);
    expect(frame.style.left).toBe('-10000px');
    const youtube = withYouTubeFrame(frame);
    const sent = vi.spyOn(frame.contentWindow!, 'postMessage');

    // The host page is ready: the tap inside YouTube's frame is asked to listen.
    post({ raygunEmbed: true, type: 'ready' }, HOST, frame.contentWindow);
    expect(sent).toHaveBeenCalledWith(
      {
        raygunCommand: true,
        type: 'tap',
        tap: 'start',
        session: 's1',
        speed: 1,
        mode: 'muted',
      },
      HOST,
    );

    // The tap found the player's video: it plays from where Rust asked, muted.
    post(
      { raygunTap: true, session: 's1', type: 'attached', speed: 1 },
      'https://www.youtube.com',
      youtube,
    );
    const commandsSent = sent.mock.calls.map(([message]) => (message as { type: string }).type);
    expect(commandsSent).toEqual(['tap', 'load', 'rate', 'mute']);
    expect(sent.mock.calls[1]![0]).toMatchObject({
      videoId: 'abc123',
      startMs: 1_500,
      autoplay: true,
    });

    // Its sound goes to Rust as the desktop capture player's does.
    const samples = new Int16Array([1, -1, 300, -300]);
    post(
      {
        raygunTap: true,
        session: 's1',
        type: 'sound',
        startMs: 1_500,
        rate: 48_000,
        channels: 2,
        speed: 1,
        pcm: samples.buffer,
        durationMs: 200_000,
        error: null,
      },
      'https://www.youtube.com',
      youtube,
    );
    await vi.waitFor(() =>
      expect(commands.songCaptureReport).toHaveBeenCalledWith({
        session: 's1',
        kind: 'sound',
        startMs: 1_500,
        rate: 48_000,
        channels: 2,
        speed: 1,
        pcm: toBase64(new Int16Array([1, -1, 300, -300])),
        durationMs: 200_000,
        error: null,
      }),
    );

    // Rust stops it: the player goes.
    act(() => emitEvent('songCaptureEvent', request({ videoId: null, embedUrl: null })));
    expect(playerFrame()).toBeNull();
    unmount();
  });

  it('only trusts the YouTube frame inside its own player', async () => {
    const { unmount } = render(<Host />);
    act(() => emitEvent('songCaptureEvent', request()));
    const frame = playerFrame()!;
    withYouTubeFrame(frame);
    const sent = vi.spyOn(frame.contentWindow!, 'postMessage');

    // Another page's YouTube frame says it found the video, then sends sound.
    const stranger = document.createElement('iframe');
    document.body.append(stranger);
    post(
      { raygunTap: true, session: 's1', type: 'attached', speed: 1 },
      'https://www.youtube.com',
      stranger.contentWindow,
    );
    post(
      { raygunTap: true, session: 's1', type: 'sound', pcm: new ArrayBuffer(4) },
      'https://www.youtube.com',
      stranger.contentWindow,
    );
    expect(sent).not.toHaveBeenCalled();
    expect(commands.songCaptureReport).not.toHaveBeenCalled();
    stranger.remove();
    unmount();
    expect(playerFrame()).toBeNull();
  });

  it('says why when YouTube refuses the video', async () => {
    const { unmount } = render(<Host />);
    act(() => emitEvent('songCaptureEvent', request()));
    const frame = playerFrame()!;
    post({ raygunEmbed: true, type: 'error', code: 150 }, HOST, frame.contentWindow);
    await vi.waitFor(() =>
      expect(commands.songCaptureReport).toHaveBeenCalledWith(
        expect.objectContaining({
          session: 's1',
          kind: 'failed',
          error: "The video's owner doesn't allow it to be played in other apps",
        }),
      ),
    );
    await vi.waitFor(() => expect(playerFrame()).toBeNull());
    unmount();
  });

  it('leaves streams to the desktop capture player', () => {
    const { unmount } = render(<Host />);
    act(() =>
      emitEvent(
        'songCaptureEvent',
        request({ videoId: null, embedUrl: null, url: 'http://127.0.0.1:1/media/x' }),
      ),
    );
    expect(playerFrame()).toBeNull();
    unmount();
  });
});
