// The slice of Nuclear's plugin model (@nuclearplayer/model and plugin-sdk, AGPL-3.0) this shim
// reads and writes. Field names follow Nuclear so plugin output converts without loss.

export type ProviderRef = { provider: string; id: string; url?: string };
export type Artwork = { url: string; width?: number; height?: number; purpose?: string };
export type ArtworkSet = { items: Artwork[] };
export type ArtistCredit = { name: string; roles: string[] };

export type NuclearTrack = {
  title: string;
  artists: ArtistCredit[];
  album?: { title: string; source?: ProviderRef };
  durationMs?: number;
  artwork?: ArtworkSet;
  source: ProviderRef;
};

export type NuclearStream = {
  url: string;
  protocol: 'file' | 'http' | 'https' | 'hls';
  qualityLabel?: string;
  bitrateKbps?: number;
};

export type NuclearStreamCandidate = {
  id: string;
  title: string;
  durationMs?: number;
  thumbnail?: string;
  stream?: NuclearStream;
  failed: boolean;
  source: ProviderRef;
};

type Segment = { text: string };
type Line = { segments: Segment[]; startMs?: number; endMs?: number };
type Section = { label?: string; lines: Line[] };

export type NuclearLyrics =
  { type: 'plain' | 'lineSynced' | 'wordSynced'; sections: Section[] } | { type: 'instrumental' };

export type NuclearProvider = {
  id: string;
  kind: string;
  name: string;
  [method: string]: unknown;
};

export type NuclearPlugin = {
  onLoad?(api: unknown): unknown;
  onEnable?(api: unknown): unknown;
  onDisable?(api: unknown): unknown;
  onUnload?(api: unknown): unknown;
};

export type PluginInfo = { id: string; name: string; version: string; categories: string[] };
