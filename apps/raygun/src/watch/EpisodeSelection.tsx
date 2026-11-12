import { Check, Download, FileDown, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ExportSheet } from '../export/ExportSheet';
import type { ExportEpisode } from '../export/ExportSheet';
import { SourceIssue } from '../shell/SourceIssue';
import type { EpisodeSelection } from './useEpisodeSelection';
import { useDownloadEpisodes } from './useVideoLibrary';

/** An episode row's box in selection mode. */
export const EpisodeCheck: FC<{ checked: boolean; name: string; onToggle: () => void }> = ({
  checked,
  name,
  onToggle,
}) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={t('watch.selectEpisode', { name })}
      onClick={onToggle}
      className="grid size-9 shrink-0 place-items-center rounded-full"
    >
      <span
        aria-hidden
        className={`grid size-5 place-items-center rounded border-2 ${checked ? 'border-primary bg-primary text-background' : 'border-muted'}`}
      >
        {checked && <Check size={14} strokeWidth={3} />}
      </span>
    </button>
  );
};

/**
 * Selection mode's bar: select all shown, download the picked episodes (each finds its own link,
 * two at a time), or export them (saved titles).
 */
export const EpisodeSelectionBar: FC<{
  selection: EpisodeSelection;
  /** The title's player group: `title:<id>` or `stremio:<type>:<id>`. */
  group: string;
  /** The episodes listed now (a season), for Select all. */
  shownIds: string[];
  /** Every episode, for the export sheet; with `titleId` (a saved title) Export shows. */
  episodes: ExportEpisode[];
  titleId: string | null;
}> = ({ selection, group, shownIds, episodes, titleId }) => {
  const { t } = useTranslation();
  const download = useDownloadEpisodes();
  const [exporting, setExporting] = useState(false);
  const ids = [...selection.selected];
  const allShown = shownIds.length > 0 && shownIds.every((id) => selection.selected.has(id));
  const button =
    'hover:bg-background flex items-center gap-2 rounded-full px-3 py-2 text-sm font-medium disabled:opacity-50';
  return (
    <div className="bg-surface motion-rise sticky bottom-2 z-30 flex flex-col gap-2 rounded-2xl p-2 shadow-xl">
      <div
        role="toolbar"
        aria-label={t('watch.selectEpisodes')}
        className="flex flex-wrap items-center gap-1"
      >
        <span role="status" className="px-2 text-sm font-semibold">
          {t('watch.selectedCount', { count: ids.length })}
        </span>
        <button
          type="button"
          className={button}
          onClick={() =>
            selection.set(
              allShown
                ? ids.filter((id) => !shownIds.includes(id))
                : [...new Set([...ids, ...shownIds])],
            )
          }
        >
          {t(allShown ? 'watch.selectNone' : 'watch.selectAllShown')}
        </button>
        <span className="flex-1" />
        <button
          type="button"
          className={button}
          disabled={ids.length === 0 || download.isPending}
          onClick={() => download.mutate({ group, episodeIds: ids })}
        >
          <Download aria-hidden size={16} />
          {t('watch.downloadSelected')}
        </button>
        {titleId && (
          <button
            type="button"
            className={button}
            disabled={ids.length === 0}
            onClick={() => setExporting(true)}
          >
            <FileDown aria-hidden size={16} />
            {t('watch.exportSelected')}
          </button>
        )}
        <button
          type="button"
          aria-label={t('watch.endSelection')}
          title={t('watch.endSelection')}
          className="hover:bg-background grid size-9 place-items-center rounded-full"
          onClick={selection.end}
        >
          <X aria-hidden size={18} />
        </button>
      </div>
      {download.isSuccess && (
        <p role="status" className="text-muted px-2 text-sm">
          {t('watch.episodesQueued', { count: download.data })}
        </p>
      )}
      {download.error && <SourceIssue compact>{download.error.message}</SourceIssue>}
      {exporting && titleId && (
        <ExportSheet
          medium="video"
          titleIds={[titleId]}
          episodes={episodes}
          initialEpisodes={ids}
          onClose={() => setExporting(false)}
        />
      )}
    </div>
  );
};
