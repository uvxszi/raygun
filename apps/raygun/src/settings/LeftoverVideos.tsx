import { commands } from '@raygun/bindings';
import type { VideoDownloadsSwept } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { FolderX } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { formatBytes } from '../lib/formatBytes';
import { PrefAction, PrefBlock } from './prefs';
import { Problem } from './Problem';

/** What a sweep removed, in words. */
const sweptText = (swept: VideoDownloadsSwept, t: ReturnType<typeof useTranslation>['t']) => {
  const lines = [
    swept.folders > 0
      ? t('storage.leftoverRemoved', { count: swept.folders, size: formatBytes(swept.bytes ?? 0) })
      : null,
    swept.entries > 0 ? t('storage.leftoverEntries', { count: swept.entries }) : null,
  ].filter(Boolean);
  return lines.length > 0 ? lines.join(' · ') : t('storage.leftoverNone');
};

/**
 * Settings → Data → Storage: removes what deleted videos left behind (`video_downloads_sweep`):
 * folders no download owns, and downloads whose files are gone. Rust also does it a while after
 * each start.
 */
export const LeftoverVideos: FC = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  // The downloads list follows Rust's own event; what's stored is read again here.
  const sweep = useMutation({
    mutationFn: () => unwrapResult(commands.videoDownloadsSweep()),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['storage'] }),
  });
  return (
    <>
      <PrefAction
        title={t('storage.leftoverVideos')}
        summary={sweep.data ? sweptText(sweep.data, t) : t('storage.leftoverVideosHint')}
        icon={FolderX}
        disabled={sweep.isPending}
        onClick={() => sweep.mutate()}
      />
      {sweep.isError && (
        <PrefBlock>
          <Problem title={t('storage.leftoverFailed')} detail={sweep.error.message} />
        </PrefBlock>
      )}
    </>
  );
};
