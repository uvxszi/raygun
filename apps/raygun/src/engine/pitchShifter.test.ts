import { phaseStep, pitchRatio, PitchStage, tapGains, wrapPhase } from './pitchShifter';

describe('the pitch shifter', () => {
  it('turns tenths of a semitone into a frequency ratio, an octave at most', () => {
    expect(pitchRatio(0)).toBe(1);
    expect(pitchRatio(120)).toBe(2);
    expect(pitchRatio(-120)).toBe(0.5);
    expect(pitchRatio(10)).toBeCloseTo(1.059_46, 4);
    expect(pitchRatio(500)).toBe(2);
  });

  it('drifts the taps through the window: shorter delays for a higher pitch', () => {
    const window = 2_400;
    expect(phaseStep(1, window)).toBe(0);
    expect(phaseStep(2, window)).toBeCloseTo(-1 / window, 10);
    expect(phaseStep(0.5, window)).toBeCloseTo(0.5 / window, 10);
    // Over a window's samples an octave up moves the phase round once.
    let phase = 0.25;
    for (let sample = 0; sample < window; sample += 1) {
      phase = wrapPhase(phase + phaseStep(2, window));
    }
    expect(phase).toBeCloseTo(0.25, 6);
  });

  it('fades the two taps on an equal-power curve, each silent where it wraps', () => {
    for (const phase of [0, 0.1, 0.25, 0.5, 0.8, 0.99]) {
      const [first, second] = tapGains(phase);
      expect(first * first + second * second).toBeCloseTo(1, 10);
    }
    expect(tapGains(0)[0]).toBe(0);
    expect(tapGains(0.5)[1]).toBeCloseTo(0, 10);
  });

  it('passes the sound straight through without a shift or a worklet', () => {
    const connections: [string, string][] = [];
    const node = (name: string) => ({
      name,
      connect(target: { name: string }) {
        connections.push([name, target.name]);
        return target;
      },
      disconnect: () => undefined,
    });
    let made = 0;
    const context = {
      currentTime: 0,
      createGain: () => node(made++ === 0 ? 'input' : 'output'),
    } as unknown as BaseAudioContext;
    const stage = new PitchStage(context);
    stage.setPitch(0);
    stage.setPitch(30);
    expect(connections).toEqual([['input', 'output']]);
  });
});
