import '@testing-library/jest-dom/vitest';
import { MotionGlobalConfig } from 'motion/react';

import '../i18n';

// Animations run instantly in tests, so exit transitions don't keep elements mounted.
MotionGlobalConfig.skipAnimations = true;

// jsdom doesn't implement media playback; the engine host still creates an <audio> element.
window.HTMLMediaElement.prototype.play = () => Promise.resolve();
window.HTMLMediaElement.prototype.pause = () => {};
window.HTMLMediaElement.prototype.load = () => {};
