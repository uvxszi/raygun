/** Reading-aloud speeds, in hundredths (`NovelReaderPrefs::tts_speed`, Rust's `TTS_SPEEDS`). */
export const SPEED_MIN = 50;
export const SPEED_MAX = 300;
export const SPEED_STEP = 5;
export const NORMAL_SPEED = 100;

/** What a tap on the reading bar's speed steps through. */
export const SPEED_CYCLE = [80, 100, 120, 150, 200] as const;

/** The speeds a hold on it offers at once, beside the slider. */
export const SPEED_BUBBLES = [50, 75, 100, 125, 150, 200, 250, 300] as const;

/** The speed a tap moves on to: the next one up, after the fastest the slowest. */
export const nextSpeed = (speed: number): number =>
  SPEED_CYCLE.find((step) => step > speed) ?? SPEED_CYCLE[0];

/** A speed as shown: "1.0×", "1.25×". */
export const speedLabel = (speed: number) => `${(speed / 100).toFixed(speed % 10 === 0 ? 1 : 2)}×`;
