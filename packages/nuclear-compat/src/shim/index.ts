// Entry of the shim bundle that goes in front of every converted Nuclear plugin. It installs the
// web polyfills and exposes `run` (see run.ts) as the global `__nuclearShim`.
import { installPolyfills } from './polyfills';
import { run } from './run';

installPolyfills();

export { run };
