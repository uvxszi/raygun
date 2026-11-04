import type { Medium } from './medium';

/**
 * The words for a library's categories: comics and books have groups, videos have folders. Pass
 * it to `t` for the `library.categories.*` keys (their `_folder` variants are the videos').
 */
export const groupWords = (medium: Medium) => ({
  context: medium === 'video' ? 'folder' : undefined,
});
