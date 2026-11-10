import { useId, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels } from '../../ai/useAi';
import { formatBytes } from '../../lib/formatBytes';
import { useDragScroll } from '../../shell/useDragScroll';
import { PrefBlock, PrefGroup, PrefInfo } from '../prefs';
import { Problem } from '../Problem';
import { ModelRow } from './ModelRows';
import { MODEL_FILTERS, MODEL_SORTS, listedModels, modelStorage } from './modelUse';
import type { ModelFilter, ModelSort } from './modelUse';

const PILL = 'shrink-0 rounded-full px-3 py-1.5 text-sm';

/** The pills that narrow the list by use (or to the downloaded ones), and its order. */
const ModelListControls: FC<{
  filter: ModelFilter;
  sort: ModelSort;
  onFilter: (filter: ModelFilter) => void;
  onSort: (sort: ModelSort) => void;
}> = ({ filter, sort, onFilter, onSort }) => {
  const { t } = useTranslation();
  const row = useDragScroll<HTMLDivElement>();
  const sortId = useId();
  return (
    <PrefBlock>
      <div className="flex flex-col gap-2">
        <div
          ref={row}
          role="group"
          aria-label={t('ai.onDevice.filter')}
          className="no-scrollbar -mx-1 flex gap-2 overflow-x-auto px-1"
        >
          {MODEL_FILTERS.map((option) => {
            const selected = filter === option;
            return (
              <button
                key={option}
                type="button"
                aria-pressed={selected}
                onClick={() => onFilter(option)}
                className={`${PILL} ${selected ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
              >
                {t(`ai.onDevice.filters.${option}`)}
              </button>
            );
          })}
        </div>
        <div className="flex items-center gap-2 self-end text-sm">
          <label htmlFor={sortId} className="text-muted">
            {t('ai.onDevice.sort')}
          </label>
          <select
            id={sortId}
            value={sort}
            onChange={(event) => onSort(event.target.value as ModelSort)}
            className="bg-surface rounded-lg px-2 py-1.5 text-sm"
          >
            {MODEL_SORTS.map((option) => (
              <option key={option} value={option}>
                {t(`ai.onDevice.sorts.${option}`)}
              </option>
            ))}
          </select>
        </div>
      </div>
    </PrefBlock>
  );
};

/**
 * On-device models: every model Raygun runs here, narrowed by pills (what it's for, or the
 * downloaded ones) and sorted by name, size or downloaded first, each with its size, state and
 * Download, Cancel or Delete; and the space the downloaded ones take.
 */
export const ModelsSection: FC = () => {
  const { t } = useTranslation();
  const models = useAiModels();
  const [filter, setFilter] = useState<ModelFilter>('all');
  const [sort, setSort] = useState<ModelSort>('name');
  const all = models.data ?? [];
  const storage = modelStorage(all);
  const shown = listedModels(all, filter, sort);

  return (
    <PrefGroup id="models" title={t('ai.onDevice.title')} summary={t('ai.onDevice.about')}>
      <PrefInfo
        title={t('ai.onDevice.storage')}
        summary={
          storage.count > 0
            ? t('ai.onDevice.storageSummary', {
                count: storage.count,
                size: formatBytes(storage.bytes),
              })
            : t('ai.onDevice.storageNone')
        }
      />
      <ModelListControls filter={filter} sort={sort} onFilter={setFilter} onSort={setSort} />
      {shown.map((model) => (
        <ModelRow key={model.id} model={model} />
      ))}
      {models.isSuccess && shown.length === 0 && (
        <PrefBlock>
          <p className="text-muted text-sm">
            {filter === 'downloaded' ? t('ai.onDevice.noneDownloaded') : t('ai.onDevice.none')}
          </p>
        </PrefBlock>
      )}
      {models.error && (
        <PrefBlock>
          <Problem title={t('ai.onDevice.listFailed')} detail={models.error.message} />
        </PrefBlock>
      )}
    </PrefGroup>
  );
};
