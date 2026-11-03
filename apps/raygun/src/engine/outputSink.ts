/**
 * Where the webview's sound comes out on desktop (M46.43): the system's outputs as the webview
 * lists them, and `setSinkId` on the players and the Web Audio graph. The choice is kept on this
 * device and applied again at start.
 */
const KEY = 'raygun.audioOutput';

type Sinkable = { setSinkId?: (id: string) => Promise<void> };

const targets = new Set<Sinkable>();

const saved = (): string => {
  try {
    return localStorage.getItem(KEY) ?? '';
  } catch {
    return '';
  }
};

let current = saved();

const apply = (target: Sinkable, id: string) => {
  void target.setSinkId?.(id).catch(() => undefined);
};

/** An element or an AudioContext the output applies to; it gets the current one at once. */
export const registerSink = (target: object) => {
  const sinkable = target as Sinkable;
  targets.add(sinkable);
  if (current) apply(sinkable, current);
};

/** Plays through output `id` ('' for the system's default). */
export const setOutputSink = (id: string) => {
  current = id;
  try {
    localStorage.setItem(KEY, id);
  } catch {
    // A per-device choice; without storage it lasts until the app closes.
  }
  for (const target of targets) apply(target, id);
};

export const outputSink = () => current;

/** The webview's outputs: `{id, name}`, the default first. Names can be blank until allowed. */
export const listOutputs = async (): Promise<{ id: string; name: string }[]> => {
  const devices = (await navigator.mediaDevices?.enumerateDevices?.().catch(() => [])) ?? [];
  return devices
    .filter((device) => device.kind === 'audiooutput')
    .map((device) => ({
      id: device.deviceId === 'default' ? '' : device.deviceId,
      name: device.label,
    }));
};
