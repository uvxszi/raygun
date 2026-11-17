import common from './fx_motion_common.glsl?raw';
import copy from './fx_motion_copy.glsl?raw';
import error from './fx_motion_error.glsl?raw';
import luma from './fx_motion_luma.glsl?raw';
import search from './fx_motion_search.glsl?raw';
import smooth from './fx_motion_smooth.glsl?raw';
import warp from './fx_motion_warp.glsl?raw';
import type { MotionSources } from '../glRenderer';

/**
 * Smooth motion's shaders as they are here, the Smooth Motion chip's package's source
 * (`tools/build_chips.py`). For tests only: the app reads them from the installed chip
 * (`motionSources.ts`), so they never ship inside it.
 */
export const IN_APP_MOTION: MotionSources = { common, copy, luma, search, smooth, error, warp };
