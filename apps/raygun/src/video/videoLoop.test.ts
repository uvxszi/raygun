import type { MediaEngine } from './mediaEngine';
import { applyLoop, loopAround, wentRound } from './videoLoop';

const engine = (kind: MediaEngine['kind']) =>
  ({ kind, keepStart: vi.fn() }) as unknown as MediaEngine;

const STREAM = 'http://127.0.0.1:1/media/t/https/cdn.example/master.m3u8';

describe('video loop', () => {
  it('loops files in the element and keeps a stream start buffered instead', () => {
    const video = document.createElement('video');
    applyLoop(video, engine('native'), true);
    expect(video.loop).toBe(true);

    const hls = engine('hls');
    applyLoop(video, hls, true);
    // Media Source streams go round by a seek: the element's own loop stays off.
    expect(video.loop).toBe(false);
    expect(hls.keepStart).toHaveBeenCalledWith(true);
    applyLoop(video, hls, false);
    expect(hls.keepStart).toHaveBeenLastCalledWith(false);
  });

  it('goes back to the start of the same source', () => {
    const video = document.createElement('video');
    video.src = STREAM;
    const play = vi.spyOn(video, 'play').mockResolvedValue();
    video.currentTime = 120;
    loopAround(video);
    expect(video.currentTime).toBe(0);
    expect(play).toHaveBeenCalledOnce();
    expect(video.getAttribute('src')).toBe(STREAM);
  });

  it('tells a loop going round from a seek', () => {
    expect(wentRound(99_400, 200, 100_000)).toBe(true);
    expect(wentRound(50_000, 200, 100_000)).toBe(false);
    expect(wentRound(99_400, 30_000, 100_000)).toBe(false);
    // Too short to tell.
    expect(wentRound(2_000, 100, 2_500)).toBe(false);
  });
});
