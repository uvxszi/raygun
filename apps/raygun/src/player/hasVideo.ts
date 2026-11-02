import type { QueueItem } from '@raygun/bindings';

/** Whether `item` plays through YouTube's embedded player, which has a picture to show. */
export const hasVideo = (item: QueueItem | undefined) => item?.source.kind === 'youTube';
