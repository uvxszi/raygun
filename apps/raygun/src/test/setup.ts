import '@testing-library/jest-dom/vitest';
import { MotionGlobalConfig } from 'motion/react';

import '../i18n';
import { resetBackLayers } from '../shell/backLayers';

// Animations run instantly in tests, so exit transitions don't keep elements mounted.
MotionGlobalConfig.skipAnimations = true;

// jsdom doesn't implement media playback; the engine host still creates an <audio> element.
window.HTMLMediaElement.prototype.play = () => Promise.resolve();
window.HTMLMediaElement.prototype.pause = () => {};
window.HTMLMediaElement.prototype.load = () => {};

// Per-device conveniences (the library's tab and filter) must not leak from one test to the next.
afterEach(() => {
  localStorage.clear();
  // Open sheets and menus are module state (shell/backLayers): each test starts with none.
  resetBackLayers();
});
