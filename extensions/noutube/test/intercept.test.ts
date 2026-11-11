// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From lib/intercept.test.ts.

import { describe, expect, it } from 'vitest';

import { blocklistFromSettings } from '../src/lib/blocklist';
import {
  filterPlayerResponse,
  transformBrowseResponse,
  transformGetWatchResponse,
  transformPlayerResponse,
  transformReelResponse,
  transformSearchResponse,
} from '../src/lib/intercept';

const blocklistOf = (channels: string[], keywords: string[]) =>
  blocklistFromSettings(channels.join(','), keywords.join(','));

describe('intercept blocklist filtering', () => {
  const blocklist = blocklistOf(['channel1'], ['spoiler']);

  it('removes search items by channel and keyword', () => {
    const response = {
      contents: {
        sectionListRenderer: {
          contents: [
            {
              itemSectionRenderer: {
                contents: [
                  {
                    videoWithContextRenderer: {
                      title: { runs: [{ text: 'Normal video' }] },
                      shortBylineText: { runs: [{ text: 'Channel1' }] },
                      navigationEndpoint: {
                        commandMetadata: { webCommandMetadata: { url: '/watch?v=1' } },
                      },
                    },
                  },
                  {
                    videoWithContextRenderer: {
                      title: { runs: [{ text: 'Spoiler explained' }] },
                      shortBylineText: { runs: [{ text: 'Other channel' }] },
                      navigationEndpoint: {
                        commandMetadata: { webCommandMetadata: { url: '/watch?v=2' } },
                      },
                    },
                  },
                  {
                    videoWithContextRenderer: {
                      title: { runs: [{ text: 'Keep me' }] },
                      shortBylineText: { runs: [{ text: 'Other channel' }] },
                      navigationEndpoint: {
                        commandMetadata: { webCommandMetadata: { url: '/watch?v=3' } },
                      },
                    },
                  },
                ],
              },
            },
          ],
        },
      },
    };

    const transformed = JSON.parse(transformSearchResponse(JSON.stringify(response), blocklist));
    const items = transformed.contents.sectionListRenderer.contents[0].itemSectionRenderer.contents;

    expect(items).toHaveLength(1);
    expect(items[0].videoWithContextRenderer.title.runs[0].text).toBe('Keep me');
  });

  const searchWithShort = (title: string) => ({
    contents: {
      sectionListRenderer: {
        contents: [
          {
            itemSectionRenderer: {
              contents: [
                {
                  videoWithContextRenderer: {
                    title: { runs: [{ text: title }] },
                    shortBylineText: { runs: [{ text: 'Other channel' }] },
                    navigationEndpoint: {
                      commandMetadata: { webCommandMetadata: { url: '/shorts/1' } },
                    },
                  },
                },
              ],
            },
          },
        ],
      },
    },
  });

  it('keeps shorts in search when only blocklist filtering is requested', () => {
    const transformed = JSON.parse(
      transformSearchResponse(JSON.stringify(searchWithShort('Keep short')), blocklist, {
        hideShorts: false,
      }),
    );
    const items = transformed.contents.sectionListRenderer.contents[0].itemSectionRenderer.contents;

    expect(items).toHaveLength(1);
  });

  it('removes matching shorts even when generic shorts filtering is off', () => {
    const transformed = JSON.parse(
      transformSearchResponse(JSON.stringify(searchWithShort('Spoiler short')), blocklist, {
        hideShorts: false,
      }),
    );
    const items = transformed.contents.sectionListRenderer.contents[0].itemSectionRenderer.contents;

    expect(items).toHaveLength(0);
  });

  const richGrid = (contents: unknown[]) => ({
    contents: {
      twoColumnBrowseResultsRenderer: {
        tabs: [{ tabRenderer: { content: { richGridRenderer: { contents } } } }],
      },
    },
  });

  const gridItems = (text: string) =>
    JSON.parse(text).contents.twoColumnBrowseResultsRenderer.tabs[0].tabRenderer.content
      .richGridRenderer.contents;

  it('removes browse rich grid items by channel and keyword', () => {
    const video = (title: string, owner: string) => ({
      richItemRenderer: {
        content: {
          videoRenderer: {
            title: { runs: [{ text: title }] },
            ownerText: { runs: [{ text: owner }] },
          },
        },
      },
    });
    const response = richGrid([
      video('Normal video', 'Channel1'),
      video('Spoiler video', 'Other channel'),
      video('Keep me', 'Other channel'),
    ]);

    const items = gridItems(transformBrowseResponse(JSON.stringify(response), blocklist));

    expect(items).toHaveLength(1);
    expect(items[0].richItemRenderer.content.videoRenderer.title.runs[0].text).toBe('Keep me');
  });

  it('removes lockupViewModel items in browse by channel name, URL, or ID', () => {
    const lockup = (name: string, browseId: string, canonicalBaseUrl: string, more = false) => ({
      richItemRenderer: {
        content: {
          lockupViewModel: {
            contentMetadata: {
              runs: [
                {
                  text: name,
                  navigationEndpoint: { browseEndpoint: { browseId, canonicalBaseUrl } },
                },
                ...(more ? [{ text: ' • ' }, { text: '1M views' }] : []),
              ],
            },
          },
        },
      },
    });
    const response = richGrid([
      lockup('BBC Music', 'UC_bbc', '/@bbcmusic', true),
      lockup('Keep me', 'UC_keep', '/@keep'),
    ]);
    const firstName = (text: string) =>
      gridItems(text)[0].richItemRenderer.content.lockupViewModel.contentMetadata.runs[0].text;

    for (const channel of ['BBC Music', '@bbcmusic', 'UC_bbc']) {
      const text = transformBrowseResponse(JSON.stringify(response), blocklistOf([channel], []));
      expect(gridItems(text)).toHaveLength(1);
      expect(firstName(text)).toBe('Keep me');
    }
  });
});

describe('intercept original title rewriting', () => {
  const player = () => ({
    videoDetails: { title: 'Original title' },
    microformat: { playerMicroformatRenderer: { title: { simpleText: 'Translated title' } } },
  });

  it('rewrites player display metadata from videoDetails title when enabled', () => {
    const transformed = JSON.parse(
      transformPlayerResponse(JSON.stringify(player()), undefined, { showOriginalTitles: true }),
    );

    expect(transformed.videoDetails.title).toBe('Original title');
    expect(transformed.microformat.playerMicroformatRenderer.title.simpleText).toBe(
      'Original title',
    );
  });

  it('leaves player display metadata translated when disabled', () => {
    const transformed = JSON.parse(transformPlayerResponse(JSON.stringify(player())));

    expect(transformed.microformat.playerMicroformatRenderer.title.simpleText).toBe(
      'Translated title',
    );
  });

  it('rewrites wrapped get_watch player metadata when enabled', () => {
    const transformed = JSON.parse(
      transformGetWatchResponse(JSON.stringify([{ playerResponse: player() }]), undefined, {
        showOriginalTitles: true,
      }),
    );

    expect(
      transformed[0].playerResponse.microformat.playerMicroformatRenderer.title.simpleText,
    ).toBe('Original title');
  });

  it('rewrites browse renderer titles only when an original title field exists', () => {
    const response = {
      contents: {
        twoColumnBrowseResultsRenderer: {
          tabs: [
            {
              tabRenderer: {
                content: {
                  richGridRenderer: {
                    contents: [
                      {
                        richItemRenderer: {
                          content: {
                            videoRenderer: {
                              title: { runs: [{ text: 'Translated title' }] },
                              originalTitle: { simpleText: 'Original title' },
                            },
                          },
                        },
                      },
                      {
                        richItemRenderer: {
                          content: {
                            videoRenderer: { title: { runs: [{ text: 'Still translated' }] } },
                          },
                        },
                      },
                    ],
                  },
                },
              },
            },
          ],
        },
      },
    };

    const transformed = JSON.parse(
      transformBrowseResponse(JSON.stringify(response), undefined, { showOriginalTitles: true }),
    );
    const items =
      transformed.contents.twoColumnBrowseResultsRenderer.tabs[0].tabRenderer.content
        .richGridRenderer.contents;

    expect(items[0].richItemRenderer.content.videoRenderer.title.runs[0].text).toBe(
      'Original title',
    );
    expect(items[1].richItemRenderer.content.videoRenderer.title.runs[0].text).toBe(
      'Still translated',
    );
  });

  it('rewrites search renderer titles while preserving shorts visibility option', () => {
    const response = {
      contents: {
        sectionListRenderer: {
          contents: [
            {
              itemSectionRenderer: {
                contents: [
                  {
                    videoWithContextRenderer: {
                      title: { runs: [{ text: 'Translated short' }] },
                      originalTitle: { runs: [{ text: 'Original short' }] },
                      shortBylineText: { runs: [{ text: 'Other channel' }] },
                      navigationEndpoint: {
                        commandMetadata: { webCommandMetadata: { url: '/shorts/1' } },
                      },
                    },
                  },
                ],
              },
            },
          ],
        },
      },
    };

    const transformed = JSON.parse(
      transformSearchResponse(JSON.stringify(response), undefined, {
        hideShorts: false,
        showOriginalTitles: true,
      }),
    );
    const items = transformed.contents.sectionListRenderer.contents[0].itemSectionRenderer.contents;

    expect(items).toHaveLength(1);
    expect(items[0].videoWithContextRenderer.title.runs[0].text).toBe('Original short');
  });
});

describe('intercept ad blocking', () => {
  it('filters Shorts across feed sections and continuations only when requested', () => {
    const ordinary = {
      videoWithContextRenderer: {
        navigationEndpoint: { commandMetadata: { webCommandMetadata: { url: '/watch?v=normal' } } },
        menu: { url: '/shorts/not-the-card' },
      },
    };
    const shorts = {
      richItemRenderer: { content: { shortsLockupViewModel: { entityId: 'short' } } },
    };
    const compact = {
      compactVideoRenderer: {
        navigationEndpoint: { commandMetadata: { webCommandMetadata: { url: '/shorts/compact' } } },
      },
    };
    const grid = {
      gridVideoRenderer: { navigationEndpoint: { reelWatchEndpoint: { videoId: 'grid' } } },
    };
    const response = {
      contents: [
        ordinary,
        shorts,
        {
          richSectionRenderer: {
            content: { gridShelfViewModel: { contents: [{ shortsLockupViewModel: {} }] } },
          },
        },
      ],
      onResponseReceivedActions: [
        {
          appendContinuationItemsAction: {
            continuationItems: [
              {
                videoWithContextRenderer: {
                  navigationEndpoint: {
                    commandMetadata: { webCommandMetadata: { url: '/shorts/abc' } },
                  },
                },
              },
              ordinary,
              compact,
              grid,
            ],
          },
        },
      ],
    };
    const continuation = (data: { onResponseReceivedActions: unknown[] }) =>
      (
        data.onResponseReceivedActions[0] as {
          appendContinuationItemsAction: { continuationItems: unknown[] };
        }
      ).appendContinuationItemsAction.continuationItems;

    const hidden = JSON.parse(
      transformBrowseResponse(JSON.stringify(response), undefined, { hideShorts: true }),
    );
    expect(hidden.contents).toEqual([ordinary]);
    expect(continuation(hidden)).toEqual([ordinary]);
    const shown = JSON.parse(
      transformBrowseResponse(JSON.stringify(response), undefined, { hideShorts: false }),
    );
    expect(shown.contents).toHaveLength(3);
    expect(continuation(shown)).toHaveLength(4);
  });

  const feed = () => ({
    contents: {
      twoColumnBrowseResultsRenderer: {
        tabs: [
          {
            tabRenderer: {
              content: {
                richGridRenderer: {
                  contents: [
                    { richItemRenderer: { content: { adSlotRenderer: { adLayoutMetadata: {} } } } },
                    {
                      richItemRenderer: {
                        content: { videoRenderer: { title: { runs: [{ text: 'Keep me' }] } } },
                      },
                    },
                  ],
                },
              },
            },
          },
        ],
      },
    },
  });

  const feedItems = (text: string) =>
    JSON.parse(text).contents.twoColumnBrowseResultsRenderer.tabs[0].tabRenderer.content
      .richGridRenderer.contents;

  it('drops ad items and player ad keys by default', () => {
    expect(feedItems(transformBrowseResponse(JSON.stringify(feed())))).toHaveLength(1);

    const player = { adPlacements: [{}], playerAds: [{}], videoDetails: { title: 'A' } };
    const transformed = JSON.parse(transformPlayerResponse(JSON.stringify(player)));
    expect(transformed.adPlacements).toBeUndefined();
    expect(transformed.playerAds).toBeUndefined();
  });

  it('keeps ads when ad blocking is off', () => {
    const items = feedItems(
      transformBrowseResponse(JSON.stringify(feed()), undefined, { blockAds: false }),
    );
    expect(items).toHaveLength(2);
    expect(items[0].richItemRenderer.content.adSlotRenderer).toBeDefined();

    const player = { adPlacements: [{}], playerAds: [{}], videoDetails: { title: 'A' } };
    const transformed = JSON.parse(
      transformPlayerResponse(JSON.stringify(player), undefined, { blockAds: false }),
    );
    expect(transformed.adPlacements).toHaveLength(1);
    expect(transformed.playerAds).toHaveLength(1);
  });

  // The watch page embeds its first player answer as an object rather than fetching it, so it
  // is filtered in place.
  it('strips the ads from an embedded player response in place', () => {
    const player: Record<string, unknown> = {
      adPlacements: [{}],
      adSlots: [{}],
      playerAds: [{}],
      adBreakHeartbeatParams: 'x',
      videoDetails: { title: 'A' },
    };
    expect(filterPlayerResponse(player)).toBe(player);
    expect(player).toEqual({ videoDetails: { title: 'A' } });

    const kept = { adPlacements: [{}], videoDetails: { title: 'A' } };
    filterPlayerResponse(kept, { blockAds: false });
    expect(kept.adPlacements).toHaveLength(1);
  });

  it('strips the ads of a get_watch answer and the ads beside the player', () => {
    const video = { videoRenderer: { title: { simpleText: 'B' } } };
    const answer = [
      { playerResponse: { adPlacements: [{}], videoDetails: { title: 'A' } } },
      { response: { contents: [{ adSlotRenderer: {} }, video] } },
    ];
    const transformed = JSON.parse(transformGetWatchResponse(JSON.stringify(answer)));
    expect(transformed[0].playerResponse).toEqual({ videoDetails: { title: 'A' } });
    expect(transformed[1].response.contents).toEqual([video]);
  });

  const reelSequence = () => ({
    entries: [
      { command: { reelWatchEndpoint: { videoId: 'short1' } } },
      { command: { reelWatchEndpoint: { videoId: 'ad1', adClientParams: { isAd: true } } } },
      { command: { reelWatchEndpoint: { videoId: 'short2', adClientParams: { isAd: false } } } },
    ],
  });

  type Reel = { command: { reelWatchEndpoint: { videoId: string } } };
  const reelIds = (root: { entries: Reel[] }) =>
    root.entries.map((entry) => entry.command.reelWatchEndpoint.videoId);

  it('drops ad entries from the shorts feed', () => {
    expect(reelIds(JSON.parse(transformReelResponse(JSON.stringify(reelSequence()))))).toEqual([
      'short1',
      'short2',
    ]);

    const wrapped = transformReelResponse(
      JSON.stringify({ reelWatchSequenceResponse: reelSequence() }),
    );
    expect(reelIds(JSON.parse(wrapped).reelWatchSequenceResponse)).toEqual(['short1', 'short2']);

    const item = {
      playerResponse: { adPlacements: [{}], adSlots: [{}], videoDetails: { title: 'A' } },
    };
    const transformed = JSON.parse(transformReelResponse(JSON.stringify(item)));
    expect(transformed.playerResponse.adPlacements).toBeUndefined();
    expect(transformed.playerResponse.adSlots).toBeUndefined();
    expect(transformed.playerResponse.videoDetails.title).toBe('A');
  });

  it('keeps the shorts feed itself when Shorts are hidden elsewhere', () => {
    const text = transformReelResponse(JSON.stringify(reelSequence()), undefined, {
      hideShorts: true,
    });
    expect(reelIds(JSON.parse(text))).toEqual(['short1', 'short2']);
  });

  it('keeps shorts feed ads when ad blocking is off', () => {
    const text = transformReelResponse(JSON.stringify(reelSequence()), undefined, {
      blockAds: false,
    });
    expect(reelIds(JSON.parse(text))).toEqual(['short1', 'ad1', 'short2']);
  });

  it('still applies the blocklist when ad blocking is off', () => {
    const items = feedItems(
      transformBrowseResponse(JSON.stringify(feed()), blocklistOf([], ['keep me']), {
        blockAds: false,
      }),
    );
    expect(items).toHaveLength(1);
    expect(items[0].richItemRenderer.content.adSlotRenderer).toBeDefined();
  });
});
