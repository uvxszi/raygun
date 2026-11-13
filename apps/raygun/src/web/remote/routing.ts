// Which commands a linked app answers for the page. The page keeps its own library, settings,
// history, player and links (synced with the apps anyway); everything else (sources, catalogues,
// searches, downloads on the app) goes to the app while one is in use. Commands the app runs only
// for itself answer `denied` and the page answers them (backend.ts remembers which).

const KEEP_PREFIXES = [
  'settings_',
  'sync_',
  'connect_',
  'devices_',
  'device_',
  'network_',
  'playback_',
  'favorite',
  'playlist',
  'collection',
  'history_',
  'recent_contexts',
  'quick_access_',
  'hidden_',
  'incognito_',
  'plugin:',
];

const KEEP = new Set(['app_info', 'storage_info', 'youtube_embed_url', 'fonts_list']);

export const goesToApp = (command: string) =>
  !KEEP.has(command) && !KEEP_PREFIXES.some((prefix) => command.startsWith(prefix));
