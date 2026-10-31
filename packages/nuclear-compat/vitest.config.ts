import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    // The dev test waits on real file watching and rebuilds.
    testTimeout: 30_000,
  },
});
