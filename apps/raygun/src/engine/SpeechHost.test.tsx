import { screen } from '@testing-library/react';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

/** jsdom has no speech: a stand-in that keeps what it was asked to say. */
class FakeUtterance {
  rate = 1;
  onstart: (() => void) | null = null;
  onend: (() => void) | null = null;
  onerror: ((event: { error: string }) => void) | null = null;
  constructor(public text: string) {}
}

const spoken: FakeUtterance[] = [];
const synth = {
  speak: vi.fn((utterance: FakeUtterance) => spoken.push(utterance)),
  cancel: vi.fn(),
};

const reports = () =>
  commands.ttsReport.mock.calls.map(([json]) => JSON.parse(json as string) as unknown);

describe('Read-aloud on desktop', () => {
  beforeEach(() => {
    spoken.length = 0;
    Object.assign(window, { speechSynthesis: synth, SpeechSynthesisUtterance: FakeUtterance });
    AppShellWrapper.seedCore();
  });

  afterEach(() => {
    delete (window as { speechSynthesis?: unknown }).speechSynthesis;
    delete (window as { SpeechSynthesisUtterance?: unknown }).SpeechSynthesisUtterance;
  });

  it('speaks the pieces Rust sends and reports where it is', async () => {
    await AppShellWrapper.mount('/');
    await screen.findByTestId('home-page');
    expect(commands.ttsSetAvailable).toHaveBeenCalledWith(true);

    emitEvent('speechCommandEvent', {
      type: 'speak',
      rate: 1.25,
      items: [
        { id: '0', text: 'Chapter one.' },
        { id: '3:end', text: 'The last line.' },
      ],
    });
    expect(synth.cancel).toHaveBeenCalled();
    expect(spoken.map((utterance) => utterance.text)).toEqual(['Chapter one.', 'The last line.']);
    expect(spoken[0]!.rate).toBe(1.25);

    spoken[0]!.onstart?.();
    spoken[0]!.onend?.();
    spoken[1]!.onstart?.();
    spoken[1]!.onend?.();
    expect(reports()).toEqual([
      { state: 'speaking', paragraph: 0 },
      { state: 'speaking', paragraph: 3 },
      { state: 'ended', paragraph: 3 },
    ]);

    // Stopping cancels; the cancelled pieces' callbacks no longer report.
    emitEvent('speechCommandEvent', { type: 'stop' });
    spoken[1]!.onerror?.({ error: 'interrupted' });
    spoken[0]!.onstart?.();
    expect(reports()).toHaveLength(3);
  });
});
