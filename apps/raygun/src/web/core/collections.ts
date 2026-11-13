// Favorites, playlists, history and what was played from, on the web: the synced rows Rust keeps
// (raygun-db collections.rs) with the same columns, stamps and order rules, so they merge with a
// phone's or a computer's when the devices sync.
import type {
  Collection,
  HistoryEntry,
  MediaSource,
  PlayContext,
  PlaylistDetail,
  PlaylistSummary,
  QueueItemInput,
  SavedSong,
  SavedTrack,
  TrackInput,
  TrackSaves,
} from '@raygun/bindings';

import { keysOf, sameSong, trackKey } from '../../music/trackKey';
import { SyncedRows, fail, uuidV7Of } from './context';
import type { CoreContext, SyncedRow } from './context';

/** Below this gap between neighbours, a move renumbers the list first. */
const MIN_POSITION_GAP = 1e-6;
/** Playlist covers are made of the first items' artwork. */
const COVER_ITEMS = 12;
const COVER_ARTWORKS = 4;
const MAX_NAME_CHARS = 100;
const RECENT_CONTEXTS = 'music.recentContexts';
const RECENT_CONTEXTS_KEPT = 24;
/** Plays from history take this many recent songs. */
const HISTORY_PLAY_LIMIT = 200;

type TrackColumns = {
  track_key: string;
  title: string;
  artist: string | null;
  album: string | null;
  duration_ms: number | null;
  artwork_url: string | null;
  source: string | null;
};

export type FavoriteRow = SyncedRow &
  TrackColumns & { id: string; added_at: number; position: number | null };
export type PlaylistRow = SyncedRow & { id: string; name: string; created_at: number };
export type ItemRow = SyncedRow &
  TrackColumns & { id: string; playlist_id: string; position: number; added_at: number };
export type HistoryRow = SyncedRow & TrackColumns & { id: string; played_at: number };

/** An http(s) address that isn't served by this device. */
export const portableUrl = (url: string) => {
  const match = /^https?:\/\/([^/?#]*)/i.exec(url);
  if (!match) return false;
  const authority = (match[1] ?? '').split('@').pop() ?? '';
  const host = (
    authority.startsWith('[')
      ? (authority.slice(1).split(']')[0] ?? '')
      : (authority.split(':')[0] ?? '')
  ).toLowerCase();
  return host !== '' && host !== 'localhost' && !host.startsWith('127.') && host !== '::1';
};

/** A source that means the same on another device. */
const portable = (source: MediaSource): MediaSource | null => {
  if (source.kind === 'url') return portableUrl(source.url) ? source : null;
  if (source.kind === 'youTube') return source;
  return null;
};

/** A source worth saving with a song: one that plays anywhere, or a lookup under other names. */
const storedSource = (source: MediaSource, title: string, artist: string | null) => {
  if (source.kind === 'lookup') {
    return source.title !== title || source.artist !== artist ? source : null;
  }
  return portable(source);
};

/** What gets stored for a track: its key, and only sources and artwork that work elsewhere. */
export const fieldsOf = (track: TrackInput): TrackColumns => {
  const source = track.source ? storedSource(track.source, track.title, track.artist) : null;
  return {
    track_key: trackKey(track.title, track.artist),
    title: track.title,
    artist: track.artist,
    album: track.album,
    // An INTEGER column in Rust's STRICT tables: a fraction there would refuse the whole sync.
    duration_ms: track.durationMs === null ? null : Math.round(track.durationMs),
    artwork_url: track.artworkUrl && portableUrl(track.artworkUrl) ? track.artworkUrl : null,
    source: source ? JSON.stringify(source) : null,
  };
};

const savedSource = (text: string | null): MediaSource | null => {
  if (!text) return null;
  try {
    const source = JSON.parse(text) as MediaSource;
    return source.kind === 'lookup' ? source : portable(source);
  } catch {
    return null;
  }
};

/** A saved song as a queue item: its stream, else a lookup for the resolver. */
export const queueItemOf = (track: SavedTrack): QueueItemInput => {
  const source: MediaSource = track.source ?? {
    kind: 'lookup',
    title: track.title,
    artist: track.artist,
    album: track.album,
    durationMs: track.durationMs,
  };
  return {
    explicit: track.explicit,
    mediaKind: source.kind === 'youTube' ? 'video' : 'audio',
    title: track.title,
    artist: track.artist,
    album: track.album,
    durationMs: track.durationMs,
    artworkUrl: track.artworkUrl,
    source,
  };
};

const playlistName = (name: string) => {
  const trimmed = [...name.trim()].slice(0, MAX_NAME_CHARS).join('');
  return trimmed || fail('A playlist needs a name');
};

/** Where a row goes so it lands before `before` (or last); null when there's no room between. */
const newPosition = (others: Array<[string, number]>, before: string | null): number | null => {
  const found = before === null ? -1 : others.findIndex(([id]) => id === before);
  const index = found === -1 ? others.length : found;
  const next = others[index]?.[1];
  const previous = index > 0 ? others[index - 1]?.[1] : undefined;
  if (previous === undefined && next === undefined) return 1;
  if (next === undefined) return (previous as number) + 1;
  if (previous === undefined) return next - 1;
  return next - previous > MIN_POSITION_GAP ? (previous + next) / 2 : null;
};

const compareIds = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);

/** A favorite's place: its position once moved, else its added time reversed (newest first). */
const favoritePlace = (row: FavoriteRow) => row.position ?? -row.added_at;

export class CollectionsService {
  private readonly rows: SyncedRows;

  constructor(private readonly context: CoreContext) {
    this.rows = new SyncedRows(context);
  }

  // ---- Reading ----

  private favoriteRows(): FavoriteRow[] {
    return this.rows
      .live<FavoriteRow>('music_favorite')
      .sort((a, b) => favoritePlace(a) - favoritePlace(b) || compareIds(b.id, a.id));
  }

  private livePlaylists(): PlaylistRow[] {
    return this.rows
      .live<PlaylistRow>('music_playlist')
      .sort(
        (a, b) =>
          a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }) ||
          compareIds(a.id, b.id),
      );
  }

  private itemsOf(playlistId: string): ItemRow[] {
    return this.rows
      .live<ItemRow>('music_playlist_item')
      .filter((item) => item.playlist_id === playlistId)
      .sort((a, b) => a.position - b.position || compareIds(a.id, b.id));
  }

  private playCounts(): Map<string, number> {
    const counts = new Map<string, number>();
    for (const row of this.rows.live<HistoryRow>('music_play_history')) {
      counts.set(row.track_key, (counts.get(row.track_key) ?? 0) + 1);
    }
    return counts;
  }

  private saved(row: TrackColumns, addedAt: number | null, counts: Map<string, number>) {
    const track: SavedTrack = {
      trackKey: row.track_key,
      title: row.title,
      artist: row.artist,
      album: row.album,
      durationMs: row.duration_ms,
      artworkUrl: row.artwork_url,
      source: savedSource(row.source),
      local: false,
      addedAt,
      playCount: counts.get(row.track_key) ?? 0,
    };
    return track;
  }

  /** Every saved copy (favorites, items of live playlists) with the keys it answers to. */
  private copies() {
    const live = new Set(this.livePlaylists().map((playlist) => playlist.id));
    const rows: Array<{ id: string; playlistId: string | null; row: TrackColumns }> = [
      ...this.rows.live<FavoriteRow>('music_favorite').map((row) => ({
        id: row.id,
        playlistId: null,
        row: row as TrackColumns,
      })),
      ...this.rows
        .live<ItemRow>('music_playlist_item')
        .filter((item) => live.has(item.playlist_id))
        .map((row) => ({ id: row.id, playlistId: row.playlist_id, row: row as TrackColumns })),
    ];
    return rows.map((copy) => ({
      ...copy,
      keys: [...new Set([copy.row.track_key, ...keysOf(copy.row.title, copy.row.artist)])],
    }));
  }

  private copiesOf(track: { title: string; artist: string | null }) {
    const wanted = keysOf(track.title, track.artist);
    return this.copies().filter((copy) =>
      copy.keys.some((key) => wanted.some((other) => sameSong(key, other))),
    );
  }

  favorites(): SavedTrack[] {
    const counts = this.playCounts();
    return this.favoriteRows().map((row) => this.saved(row, row.added_at, counts));
  }

  isFavorite(track: TrackInput): boolean {
    return this.copiesOf(track).some((copy) => copy.playlistId === null);
  }

  saves(track: TrackInput): TrackSaves {
    const copies = this.copiesOf(track);
    return {
      favorite: copies.some((copy) => copy.playlistId === null),
      playlistIds: [
        ...new Set(copies.flatMap((copy) => (copy.playlistId ? [copy.playlistId] : []))),
      ],
    };
  }

  savedIndex(): SavedSong[] {
    const songs = new Map<string, SavedSong>();
    for (const copy of this.copies()) {
      const song = songs.get(copy.row.track_key) ?? {
        trackKey: copy.row.track_key,
        songKeys: [],
        favorite: false,
        playlistIds: [],
      };
      song.songKeys = [...new Set([...song.songKeys, ...copy.keys])];
      if (copy.playlistId === null) song.favorite = true;
      else if (!song.playlistIds.includes(copy.playlistId)) song.playlistIds.push(copy.playlistId);
      songs.set(copy.row.track_key, song);
    }
    return [...songs.values()];
  }

  savedKeys(): string[] {
    return [...new Set(this.copies().flatMap((copy) => copy.keys))];
  }

  private summary(playlist: PlaylistRow): PlaylistSummary {
    const items = this.itemsOf(playlist.id);
    const artworkUrls: string[] = [];
    for (const item of items.slice(0, COVER_ITEMS)) {
      if (item.artwork_url && !artworkUrls.includes(item.artwork_url)) {
        artworkUrls.push(item.artwork_url);
      }
      if (artworkUrls.length === COVER_ARTWORKS) break;
    }
    const known = items.filter((item) => item.duration_ms !== null);
    return {
      id: playlist.id,
      name: playlist.name,
      itemCount: items.length,
      artworkUrls,
      coverUrl: null,
      length: {
        knownSecs: Math.round(known.reduce((sum, item) => sum + (item.duration_ms ?? 0), 0) / 1000),
        knownCount: known.length,
      },
    };
  }

  playlists(): PlaylistSummary[] {
    return this.livePlaylists().map((playlist) => this.summary(playlist));
  }

  private livePlaylist(id: string): PlaylistRow {
    const playlist = this.rows.get<PlaylistRow>('music_playlist', id);
    return playlist && playlist.deleted === 0 ? playlist : fail('This playlist no longer exists');
  }

  playlist(id: string): PlaylistDetail {
    const playlist = this.livePlaylist(id);
    const counts = this.playCounts();
    return {
      playlist: this.summary(playlist),
      items: this.itemsOf(id).map((item) => ({
        id: item.id,
        track: this.saved(item, item.added_at, counts),
      })),
    };
  }

  history(limit: number): HistoryEntry[] {
    const latest = new Map<string, HistoryRow>();
    for (const row of this.rows.live<HistoryRow>('music_play_history')) {
      const held = latest.get(row.track_key);
      if (!held || row.played_at > held.played_at) latest.set(row.track_key, row);
    }
    const counts = this.playCounts();
    return [...latest.values()]
      .sort((a, b) => b.played_at - a.played_at || compareIds(b.id, a.id))
      .slice(0, limit)
      .map((row) => ({ track: this.saved(row, null, counts), playedAt: row.played_at }));
  }

  recentContexts(): PlayContext[] {
    return this.context.meta<PlayContext[]>(RECENT_CONTEXTS) ?? [];
  }

  // ---- Writing ----

  private writeFavorite(fields: TrackColumns, addedAt: number, stamp: bigint) {
    const rows = this.rows.all<FavoriteRow>('music_favorite');
    if (rows.some((row) => row.track_key === fields.track_key && row.deleted === 0)) return;
    // Favoriting again revives the tombstone, so a song keeps one id across devices.
    const dead = rows
      .filter((row) => row.track_key === fields.track_key)
      .sort((a, b) => (a.updated_at_hlc > b.updated_at_hlc ? -1 : 1))[0];
    this.rows.write(
      'music_favorite',
      {
        id: dead?.id ?? uuidV7Of(this.context),
        ...fields,
        added_at: addedAt,
        position: null,
        deleted: 0,
      },
      stamp,
    );
  }

  private tombstone(table: 'music_favorite' | 'music_playlist_item', id: string, stamp: bigint) {
    const row = this.rows.get<SyncedRow>(table, id);
    if (row && row.deleted === 0) this.rows.write(table, { ...row, deleted: 1 }, stamp);
  }

  setFavorite(track: TrackInput, favorite: boolean) {
    this.setFavorites([track], favorite);
  }

  /** Favorites several songs (the first is the newest), or takes every copy out of Favorites. */
  setFavorites(tracks: TrackInput[], favorite: boolean) {
    const stamp = this.rows.stamp();
    if (!favorite) {
      for (const track of tracks) {
        for (const copy of this.copiesOf(track)) {
          if (copy.playlistId === null) this.tombstone('music_favorite', copy.id, stamp);
        }
      }
      return;
    }
    const now = this.context.now();
    tracks.forEach((track, place) => {
      if (this.isFavorite(track)) return;
      this.writeFavorite(fieldsOf(track), now - place, stamp);
    });
  }

  /** Makes a song saved exactly where `saves` says. */
  setSaves(track: TrackInput, saves: TrackSaves) {
    const copies = this.copiesOf(track);
    const stamp = this.rows.stamp();
    for (const copy of copies) {
      if (
        copy.playlistId === null ? !saves.favorite : !saves.playlistIds.includes(copy.playlistId)
      ) {
        this.tombstone(
          copy.playlistId === null ? 'music_favorite' : 'music_playlist_item',
          copy.id,
          stamp,
        );
      }
    }
    const fields = fieldsOf(track);
    // A new copy plays what the others do.
    if (!fields.source) fields.source = copies.find((copy) => copy.row.source)?.row.source ?? null;
    const now = this.context.now();
    if (saves.favorite && !copies.some((copy) => copy.playlistId === null)) {
      this.writeFavorite(fields, now, stamp);
    }
    for (const id of saves.playlistIds) {
      if (copies.some((copy) => copy.playlistId === id)) continue;
      const playlist = this.rows.get<PlaylistRow>('music_playlist', id);
      if (playlist && playlist.deleted === 0) this.appendItems(id, [fields], now, stamp);
    }
  }

  private appendItems(playlistId: string, fields: TrackColumns[], now: number, stamp: bigint) {
    let position = this.itemsOf(playlistId).reduce(
      (last, item) => Math.max(last, item.position),
      0,
    );
    for (const track of fields) {
      position += 1;
      this.rows.write(
        'music_playlist_item',
        {
          id: uuidV7Of(this.context),
          playlist_id: playlistId,
          position,
          ...track,
          added_at: now,
          deleted: 0,
        },
        stamp,
      );
    }
  }

  createPlaylist(name: string): PlaylistSummary {
    const row = this.rows.write('music_playlist', {
      id: uuidV7Of(this.context),
      name: playlistName(name),
      created_at: this.context.now(),
      deleted: 0,
    }) as unknown as PlaylistRow;
    return this.summary(row);
  }

  renamePlaylist(id: string, name: string) {
    const playlist = this.livePlaylist(id);
    this.rows.write('music_playlist', { ...playlist, name: playlistName(name) });
  }

  deletePlaylist(id: string) {
    const playlist = this.rows.get<PlaylistRow>('music_playlist', id);
    if (!playlist || playlist.deleted !== 0) return;
    const stamp = this.rows.stamp();
    this.rows.write('music_playlist', { ...playlist, deleted: 1 }, stamp);
    for (const item of this.itemsOf(id)) this.tombstone('music_playlist_item', item.id, stamp);
  }

  addToPlaylist(id: string, tracks: TrackInput[], duplicates: boolean) {
    this.livePlaylist(id);
    const adding: TrackColumns[] = [];
    let already = 0;
    for (const track of tracks) {
      const copies = this.copiesOf(track);
      if (!duplicates && copies.some((copy) => copy.playlistId === id)) {
        already += 1;
        continue;
      }
      const fields = fieldsOf(track);
      if (!fields.source)
        fields.source = copies.find((copy) => copy.row.source)?.row.source ?? null;
      adding.push(fields);
    }
    if (adding.length > 0) this.appendItems(id, adding, this.context.now(), this.rows.stamp());
    return { added: adding.length, already };
  }

  mergePlaylists(ids: string[], name: string, skipDuplicates: boolean, removeMerged: boolean) {
    if (ids.length < 2) fail('Pick at least two playlists');
    const fields: TrackColumns[] = [];
    const seen: string[][] = [];
    for (const id of ids) {
      for (const item of this.itemsOf(id)) {
        const keys = [item.track_key, ...keysOf(item.title, item.artist)];
        const known = seen.some((other) =>
          other.some((key) => keys.some((mine) => sameSong(key, mine))),
        );
        if (skipDuplicates && known) continue;
        seen.push(keys);
        fields.push(item);
      }
    }
    const created = this.createPlaylist(name);
    if (fields.length > 0) {
      const plain = fields.map((item) => ({
        track_key: item.track_key,
        title: item.title,
        artist: item.artist,
        album: item.album,
        duration_ms: item.duration_ms,
        artwork_url: item.artwork_url,
        source: item.source,
      }));
      this.appendItems(created.id, plain, this.context.now(), this.rows.stamp());
    }
    if (removeMerged) for (const id of ids) this.deletePlaylist(id);
    return this.summary(this.livePlaylist(created.id));
  }

  removePlaylistItem(itemId: string) {
    this.tombstone('music_playlist_item', itemId, this.rows.stamp());
  }

  private placeRow(
    table: 'music_favorite' | 'music_playlist_item',
    id: string,
    before: string | null,
    others: Array<[string, number]>,
  ) {
    const stamp = this.rows.stamp();
    const setPosition = (rowId: string, position: number) => {
      const row = this.rows.get<SyncedRow>(table, rowId);
      if (row) this.rows.write(table, { ...row, position }, stamp);
    };
    let position = newPosition(others, before);
    if (position === null) {
      others.forEach((other, index) => {
        other[1] = index + 1;
        setPosition(other[0], index + 1);
      });
      position = newPosition(others, before);
    }
    if (position !== null) setPosition(id, position);
  }

  private setOrder(
    table: 'music_favorite' | 'music_playlist_item',
    current: Array<[string, number]>,
    ids: string[],
  ) {
    const live = new Set(current.map(([id]) => id));
    const order: string[] = [];
    for (const id of [...ids, ...current.map(([id]) => id)]) {
      if (live.has(id) && !order.includes(id)) order.push(id);
    }
    const stamp = this.rows.stamp();
    order.forEach((id, index) => {
      const position = index + 1;
      const was = current.find(([other]) => other === id)?.[1];
      const row = this.rows.get<SyncedRow>(table, id);
      if (row && was !== position) this.rows.write(table, { ...row, position }, stamp);
    });
  }

  movePlaylistItem(itemId: string, beforeItemId: string | null) {
    const item = this.rows.get<ItemRow>('music_playlist_item', itemId);
    if (!item || item.deleted !== 0) return;
    const others = this.itemsOf(item.playlist_id)
      .filter((other) => other.id !== itemId)
      .map((other): [string, number] => [other.id, other.position]);
    this.placeRow('music_playlist_item', itemId, beforeItemId, others);
  }

  setPlaylistOrder(playlistId: string, itemIds: string[]) {
    const current = this.itemsOf(playlistId).map((item): [string, number] => [
      item.id,
      item.position,
    ]);
    this.setOrder('music_playlist_item', current, itemIds);
  }

  private favoritePlaces(): Array<[string, string, number]> {
    return this.favoriteRows().map((row) => [row.id, row.track_key, favoritePlace(row)]);
  }

  moveFavorite(key: string, beforeKey: string | null) {
    const places = this.favoritePlaces();
    const idOf = (wanted: string) => places.find(([, other]) => other === wanted)?.[0] ?? null;
    const id = idOf(key);
    if (id === null) return;
    const before = beforeKey === null ? null : idOf(beforeKey);
    const others = places
      .filter(([other]) => other !== id)
      .map(([other, , place]): [string, number] => [other, place]);
    this.placeRow('music_favorite', id, before, others);
  }

  setFavoritesOrder(keys: string[]) {
    const places = this.favoritePlaces();
    const ids = keys.flatMap((key) => {
      const found = places.find(([, other]) => other === key);
      return found ? [found[0]] : [];
    });
    this.setOrder(
      'music_favorite',
      places.map(([id, , place]): [string, number] => [id, place]),
      ids,
    );
  }

  clearHistory() {
    const stamp = this.rows.stamp();
    for (const row of this.rows.live<HistoryRow>('music_play_history')) {
      this.rows.write('music_play_history', { ...row, deleted: 1 }, stamp);
    }
  }

  /** Appends a play (the playback service decides when a song counts as played). */
  recordPlay(track: TrackInput) {
    this.rows.write('music_play_history', {
      id: uuidV7Of(this.context),
      ...fieldsOf(track),
      played_at: this.context.now(),
      deleted: 0,
    });
  }

  addRecentContext(context: PlayContext) {
    if (!context.name.trim() || !context.to.startsWith('/')) {
      fail('A play context needs a name and a page');
    }
    const list = this.recentContexts().filter(
      (other) =>
        other.to !== context.to || JSON.stringify(other.search) !== JSON.stringify(context.search),
    );
    list.unshift({ ...context, playedAt: this.context.now() });
    this.context.setMeta(RECENT_CONTEXTS, list.slice(0, RECENT_CONTEXTS_KEPT));
  }

  /** The songs a collection plays, in the order shown (`order`: the table's keys). */
  tracksOf(collection: Collection, order?: string[] | null): SavedTrack[] {
    let tracks: SavedTrack[];
    switch (collection.kind) {
      case 'favorites':
        tracks = this.favorites();
        break;
      case 'playlist':
        tracks = this.playlist(collection.id).items.map((item) => item.track);
        break;
      case 'history':
        tracks = this.history(HISTORY_PLAY_LIMIT).map((entry) => entry.track);
        break;
      case 'downloaded':
        tracks = [];
        break;
    }
    if (!order) return tracks;
    const left: Array<SavedTrack | null> = [...tracks];
    return order.flatMap((key) => {
      const index = left.findIndex((track) => track?.trackKey === key);
      if (index === -1) return [];
      const track = left[index] as SavedTrack;
      left[index] = null;
      return [track];
    });
  }
}
