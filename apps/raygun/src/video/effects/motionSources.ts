import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../../lib/commandResult';
import type { MotionSources } from './glRenderer';

/** Each of smooth motion's shaders by its file in the Smooth Motion chip. */
export const MOTION_FILES: Record<keyof MotionSources, string> = {
  common: 'fx_motion_common.glsl',
  copy: 'fx_motion_copy.glsl',
  luma: 'fx_motion_luma.glsl',
  search: 'fx_motion_search.glsl',
  smooth: 'fx_motion_smooth.glsl',
  error: 'fx_motion_error.glsl',
  warp: 'fx_motion_warp.glsl',
};

/** Reads smooth motion's shaders from the installed Smooth Motion chip. */
export const loadMotionSources = async (): Promise<MotionSources> => {
  const entries = await Promise.all(
    Object.entries(MOTION_FILES).map(async ([part, file]) => [
      part,
      await unwrapResult(commands.chipsReadText('smoothMotion', file)),
    ]),
  );
  return Object.fromEntries(entries) as MotionSources;
};

/** Smooth motion's shaders while its chip is installed and on (`enabled`); otherwise none. */
export const useMotionSources = (enabled: boolean): MotionSources | null => {
  const sources = useQuery({
    queryKey: ['chips', 'smoothMotion', 'shaders'],
    queryFn: loadMotionSources,
    enabled,
    staleTime: Infinity,
    retry: false,
  });
  return enabled ? (sources.data ?? null) : null;
};
