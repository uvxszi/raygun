import { NO_SMART_AUDIO } from './effectParams';
import {
  LEVEL_RANGE_DB,
  LoudnessFollower,
  LoudnessHistogram,
  lufs,
  NORMALIZE_RANGE_DB,
  TARGET_LUFS,
} from './smartAudio';

/** A step's power for a loudness, LUFS. */
const power = (loudness: number) => 10 ** ((loudness + 0.691) / 10);

const run = (
  follower: LoudnessFollower,
  loudness: number,
  seconds: number,
  audio = NO_SMART_AUDIO,
) => {
  for (let step = 0; step < seconds * 10; step += 1) follower.step(power(loudness), audio);
};

describe('even loudness and scene leveling', () => {
  it('turn a quiet video up to the target, within their range', () => {
    const follower = new LoudnessFollower();
    run(follower, -30, 20, { ...NO_SMART_AUDIO, normalize: true });
    expect(follower.integrated).toBeCloseTo(-30, 1);
    expect(follower.normalizeDb).toBeCloseTo(TARGET_LUFS + 30, 1);

    const quiet = new LoudnessFollower();
    run(quiet, -50, 30, { ...NO_SMART_AUDIO, normalize: true });
    expect(quiet.normalizeDb).toBe(NORMALIZE_RANGE_DB);
  });

  it('hold through silence and bring a loud scene down', () => {
    const follower = new LoudnessFollower();
    const leveling = { ...NO_SMART_AUDIO, leveling: true };
    run(follower, -90, 5, leveling);
    expect(follower.levelDb).toBe(0);
    run(follower, -6, 10, leveling);
    expect(follower.levelDb).toBe(-LEVEL_RANGE_DB);
    // Back to a quiet scene: up again, slowly.
    run(follower, -30, 2, leveling);
    expect(follower.levelDb).toBeGreaterThan(-LEVEL_RANGE_DB);
    expect(follower.levelDb).toBeLessThan(0);
  });

  it('glide back to nothing when turned off', () => {
    const follower = new LoudnessFollower();
    run(follower, -6, 10, { ...NO_SMART_AUDIO, leveling: true });
    run(follower, -6, 5);
    expect(follower.levelDb).toBe(0);
  });

  it('gate silence out of the integrated loudness', () => {
    const histogram = new LoudnessHistogram();
    for (let block = 0; block < 100; block += 1) histogram.add(1e-3);
    for (let block = 0; block < 100; block += 1) histogram.add(1e-12);
    expect(histogram.integrated()).toBeCloseTo(lufs(1e-3), 1);
    expect(histogram.seconds).toBe(10);
  });
});
