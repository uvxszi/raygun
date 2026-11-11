// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From lib/blocklist.test.ts.

import { describe, expect, it } from 'vitest';

import {
  blocklistChannelMatches,
  blocklistFromSettings,
  blocklistTextMatches,
  isBlocklistEmpty,
  normalizeBlocklistValue,
  parseBlocklistEntries,
} from '../src/lib/blocklist';

describe('blocklist', () => {
  it('normalizes values for matching', () => {
    expect(normalizeBlocklistValue(' @Channel1 ')).toBe('channel1');
    expect(normalizeBlocklistValue('https://www.youtube.com/@Channel1')).toBe('channel1');
    expect(normalizeBlocklistValue('https://www.youtube.com/c/Channel1')).toBe('channel1');
    expect(normalizeBlocklistValue('https://www.youtube.com/channel/UC12345')).toBe('uc12345');
  });

  it('reads a setting, dropping blanks and repeats', () => {
    expect(parseBlocklistEntries('Channel1, channel1 ,  , @Channel1\nSpoiler')).toEqual([
      'channel1',
      'spoiler',
    ]);
    expect(isBlocklistEmpty(blocklistFromSettings(' , ', ''))).toBe(true);
    expect(isBlocklistEmpty(blocklistFromSettings('', 'drama'))).toBe(false);
  });

  it('matches title words case-insensitively', () => {
    const { keywords } = blocklistFromSettings('', 'Final Boss');

    expect(blocklistTextMatches('My FINAL BOSS guide', keywords)).toBe(true);
    expect(blocklistTextMatches('Daily drama update', keywords)).toBe(false);
  });

  it('matches channels exactly case-insensitively', () => {
    const { channels } = blocklistFromSettings('Channel1, Google', '');

    expect(blocklistChannelMatches('Channel1', channels)).toBe(true);
    expect(blocklistChannelMatches('channel1', channels)).toBe(true);
    expect(blocklistChannelMatches('Google Developers', channels)).toBe(false);
    expect(blocklistChannelMatches('UC', channels)).toBe(false);
  });
});
