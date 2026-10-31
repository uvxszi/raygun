import '@testing-library/jest-dom/vitest';
import { MotionGlobalConfig } from 'motion/react';

import '../i18n';

// Animations run instantly in tests, so exit transitions don't keep elements mounted.
MotionGlobalConfig.skipAnimations = true;
