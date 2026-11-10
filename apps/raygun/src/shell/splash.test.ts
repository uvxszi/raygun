import { fireEvent, renderHook, waitFor } from '@testing-library/react';

import { SPLASH_ID, hideSplash, useHideSplash } from './splash';

/** The start screen as index.html has it. */
const addSplash = () => {
  const splash = document.createElement('div');
  splash.id = SPLASH_ID;
  document.body.append(splash);
  return splash;
};

describe('The start screen', () => {
  afterEach(() => {
    document.getElementById(SPLASH_ID)?.remove();
    delete document.documentElement.dataset.motion;
    vi.useRealTimers();
  });

  it('stays until the shell is ready, then goes after the next frame', async () => {
    const splash = addSplash();
    document.documentElement.dataset.motion = 'off';
    const { rerender } = renderHook(({ ready }) => useHideSplash(ready), {
      initialProps: { ready: false },
    });
    await new Promise((done) => setTimeout(done, 50));
    expect(splash).toBeInTheDocument();

    rerender({ ready: true });
    await waitFor(() => expect(splash).not.toBeInTheDocument());
  });

  it('fades out where things may move, and is gone once the fade ends', () => {
    const splash = addSplash();
    document.documentElement.dataset.motion = 'lively';
    hideSplash();
    expect(splash).toHaveAttribute('data-leaving');
    expect(splash).toBeInTheDocument();
    fireEvent.transitionEnd(splash);
    expect(splash).not.toBeInTheDocument();
  });

  it("doesn't wait for a fade that never ends", () => {
    vi.useFakeTimers();
    const splash = addSplash();
    document.documentElement.dataset.motion = 'calm';
    hideSplash();
    vi.advanceTimersByTime(300);
    expect(splash).not.toBeInTheDocument();
  });

  it.each(['reduced', 'off'])('goes at once without a fade where motion is %s', (motion) => {
    const splash = addSplash();
    document.documentElement.dataset.motion = motion;
    hideSplash();
    expect(splash).not.toBeInTheDocument();
  });

  it('goes at once when asked to (the miniplayer)', () => {
    const splash = addSplash();
    document.documentElement.dataset.motion = 'lively';
    hideSplash({ fade: false });
    expect(splash).not.toBeInTheDocument();
  });
});
