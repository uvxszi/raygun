import type { TFunction } from 'i18next';

import type { TorrentTransfer } from './protocol';

/** "12 peers · 1.4 MB/s" for a torrent playing (Rust's `transfer` updates), or that peers are
 * being found; with Data Saver's note while it applies. */
export const transferText = (transfer: TorrentTransfer | null, t: TFunction): string | null => {
  if (!transfer) return null;
  const parts = [
    transfer.peers === 0
      ? t('videoPlayer.torrentFindingPeers')
      : t('videoPlayer.torrentPeers', { count: transfer.peers, n: transfer.peers }),
  ];
  if (transfer.downloadBps > 0) {
    parts.push(
      transfer.downloadBps >= 1_000_000
        ? `${(transfer.downloadBps / 1e6).toFixed(1)} MB/s`
        : `${Math.max(1, Math.round(transfer.downloadBps / 1000))} kB/s`,
    );
  }
  if (transfer.dataSaver) parts.push(t('videoPlayer.torrentDataSaver'));
  return parts.join(' · ');
};
