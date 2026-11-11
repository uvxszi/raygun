// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/sponsorblock.ts and the skipping in content/player.ts.
//
// Sponsored segments come from SponsorBlock's open API (sponsor.ajay.app), asked by the first
// characters of the video id's SHA-256 hash, so the service doesn't learn which video plays.

import { isObject, pickArray, pickString } from './lib/json';
import { pageFetch } from './page';
import {
  getPlayer,
  onTimeUpdate,
  onVideoChange,
  playerVideoId,
  playsMainVideo,
  type Player,
} from './player';

type Segment = {
  // https://wiki.sponsor.ajay.app/w/Types#Category
  category: string;
  actionType: string;
  segment: [number, number];
};

const API = 'https://sponsor.ajay.app/api/skipSegments/';

// https://wiki.sponsor.ajay.app/w/Types#Category
const categoryColors: Record<string, string> = {
  sponsor: '#00d400',
  selfpromo: '#ffff00',
  interaction: '#cc00ff',
  intro: '#00ffff',
  outro: '#0202ed',
  preview: '#008fd6',
  music_offtopic: '#ff9900',
  poi_highlight: '#ff1684',
  filler: '#7300ff',
};

const overlayClass = '_nou_sb_segments';
const segmentClass = '_nou_sb_segment';

// The bar spanning the whole duration: the new mobile layout's, the desktop's, the older mobile one's.
const progressBarSelectors = [
  '.ytChapteredProgressBarHost',
  '.ytp-progress-list',
  '.progress-bar-line',
  '.progress-bar-background',
];
// Chaptered videos split the bar into a block per chapter with gaps between, so a segment is
// drawn piece by piece to stay over the time it covers.
const chapterSelectors = [
  '.ytChapteredProgressBarChapteredPlayerBarChapter',
  '.ytp-chapter-hover-container',
];

type Chapter = { start: number; end: number; left: number; width: number };

function getProgressBars() {
  const bars: HTMLElement[] = [];
  for (const selector of progressBarSelectors) {
    for (const element of document.querySelectorAll<HTMLElement>(selector)) {
      if (element.offsetWidth > 0) bars.push(element);
    }
  }
  return bars;
}

function getChapters(bar: HTMLElement): Chapter[] {
  let elements: HTMLElement[] = [];
  for (const selector of chapterSelectors) {
    elements = Array.from(bar.querySelectorAll<HTMLElement>(selector));
    if (elements.length) break;
  }

  if (!elements.length) return [{ start: 0, end: 1, left: 0, width: bar.offsetWidth }];

  // The gaps make the drawn widths a little narrower than the time they stand for, so the span
  // comes from the inline percentage when YouTube gives one, else from the widths.
  const totalWidth = elements.reduce((sum, element) => sum + element.offsetWidth, 0);
  const chapters: Chapter[] = [];
  let start = 0;
  for (const element of elements) {
    const percent = parseFloat(element.style.width);
    const span =
      element.style.width.endsWith('%') && Number.isFinite(percent)
        ? percent / 100
        : element.offsetWidth / (totalWidth || 1);
    chapters.push({
      start,
      end: start + span,
      left: element.offsetLeft,
      width: element.offsetWidth,
    });
    start += span;
  }
  // Rounding drift is absorbed, so the last chapter always reaches the end.
  const last = chapters[chapters.length - 1];
  if (last) last.end = 1;
  return chapters;
}

function clearSkipSegments() {
  for (const element of document.querySelectorAll(`.${overlayClass}`)) {
    element.parentElement?.removeAttribute('data-nou-sb');
    element.remove();
  }
}

function renderSkipSegments(videoId: string, segments: Segment[], duration: number) {
  if (!videoId || !duration || !Number.isFinite(duration)) return;

  const segmentKey = segments
    .map(
      ({ actionType, category, segment }) =>
        `${actionType}:${category}:${segment[0]}:${segment[1]}`,
    )
    .join(',');
  for (const bar of getProgressBars()) {
    const overlay = bar.querySelector<HTMLElement>(`.${overlayClass}`);
    const chapters = getChapters(bar);
    const chapterKey = chapters
      .map(({ start, end, left, width }) => `${start}:${end}:${left}:${width}`)
      .join(',');
    const key = `${videoId}:${duration}:${segmentKey}:${bar.offsetWidth}:${chapterKey}`;
    // YouTube redraws and resizes its controls, so the segments are drawn again whenever the
    // overlay went or the segments or the chapters' layout changed.
    if (overlay && bar.dataset.nouSb === key) continue;
    overlay?.remove();
    // Chapter offsets are measured from the bar, so it has to be the offset parent.
    if (getComputedStyle(bar).position == 'static') bar.style.position = 'relative';
    bar.append(buildOverlay(segments, duration, chapters));
    bar.dataset.nouSb = key;
  }
}

function buildOverlay(segments: Segment[], duration: number, chapters: Chapter[]) {
  const overlay = document.createElement('div');
  overlay.className = overlayClass;

  for (const segment of segments) {
    // A `full` segment covers the whole video, so there is nothing to point at.
    if (segment.actionType == 'full') continue;
    const background = categoryColors[segment.category] || categoryColors.sponsor || '';
    const start = Math.max(0, Math.min(1, segment.segment[0] / duration));
    const end = Math.max(start, Math.min(1, segment.segment[1] / duration));

    for (const chapter of chapters) {
      const from = Math.max(start, chapter.start);
      const to = Math.min(end, chapter.end);
      // Categories without length, such as poi_highlight, still get a mark.
      if (start == end ? to < from : to <= from) continue;
      const span = chapter.end - chapter.start;
      if (span <= 0) continue;
      // Summed chapter spans drift by a rounding error, which would leave a sliver of a segment
      // in the chapter after the one it really ends in.
      const width = ((to - from) / span) * chapter.width;
      if (start != end && width < 0.5) continue;
      const element = document.createElement('div');
      element.className = segmentClass;
      element.style.left = `${chapter.left + ((from - chapter.start) / span) * chapter.width}px`;
      element.style.width = `${Math.max(start == end ? 2 : 1, width)}px`;
      element.style.background = background;
      overlay.append(element);
    }
  }

  return overlay;
}

const toSegment = (value: unknown): Segment | undefined => {
  const range = pickArray(value, 'segment');
  const [from, to] = range;
  if (typeof from !== 'number' || typeof to !== 'number') return undefined;
  return {
    category: pickString(value, 'category') ?? 'sponsor',
    actionType: pickString(value, 'actionType') ?? 'skip',
    segment: [from, to],
  };
};

async function getSkipSegments(videoId: string): Promise<Segment[]> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(videoId));
  const prefix = Array.from(new Uint8Array(digest).slice(0, 2), (byte) =>
    byte.toString(16).padStart(2, '0'),
  ).join('');
  // Sponsored segments that are skipped (not those marked to be muted only).
  const query = `categories=${encodeURIComponent('["sponsor"]')}&actionTypes=${encodeURIComponent('["skip"]')}`;
  const response = await pageFetch(`${API}${prefix}?${query}`);
  // 404: no segments for any video under this prefix.
  if (response.status !== 200) return [];
  const videos: unknown = await response.json();
  const video = (Array.isArray(videos) ? videos : []).find(
    (entry: unknown) => isObject(entry) && entry.videoID === videoId,
  );
  return pickArray(video, 'segments')
    .map(toSegment)
    .filter((segment): segment is Segment => segment !== undefined);
}

let segmentsFor = '';
let segments: Segment[] = [];
let requested = '';

function load(player: Player) {
  const videoId = playerVideoId(player);
  if (!videoId || videoId === requested || !playsMainVideo(player)) return;
  requested = videoId;
  segmentsFor = '';
  segments = [];
  clearSkipSegments();
  getSkipSegments(videoId)
    .then((found) => {
      // The listener may have moved on to another video meanwhile.
      if (requested !== videoId) return;
      segmentsFor = videoId;
      segments = found;
      redraw();
    })
    .catch((error: unknown) => {
      console.warn('[YouTube enhancements] SponsorBlock:', error);
    });
}

const durationOf = (player: Player) => {
  try {
    return player.getDuration?.() ?? 0;
  } catch {
    return 0;
  }
};

function redraw() {
  const player = getPlayer();
  if (!player || !segments.length || playerVideoId(player) !== segmentsFor) return;
  renderSkipSegments(segmentsFor, segments, durationOf(player));
}

export function installSponsorBlock() {
  onVideoChange((_video, player) => {
    clearSkipSegments();
    if (player) load(player);
  });

  let lastDrawn = 0;
  onTimeUpdate((player, media) => {
    // The player can move on without a state change the watcher hears (a preview opened as the
    // page's video), so each update checks it has this video's segments.
    load(player);
    if (playerVideoId(player) !== segmentsFor || !segments.length) return;
    const now = Date.now();
    if (now - lastDrawn > 1000) {
      lastDrawn = now;
      renderSkipSegments(segmentsFor, segments, durationOf(player));
    }
    const time = media.currentTime;
    for (const { actionType, segment } of segments) {
      if (actionType !== 'skip') continue;
      const [start, end] = segment;
      if (time > start && time < end) {
        player.seekTo?.(end, true);
        return;
      }
    }
  });

  // Full screen and rotation resize the bar, and a paused video gets no updates to redraw on.
  window.addEventListener('resize', redraw);
  document.addEventListener('fullscreenchange', () => {
    redraw();
    setTimeout(redraw, 400);
  });
}
