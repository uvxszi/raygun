import type { AiModel } from '@raygun/bindings';
import { Download, LoaderCircle, Trash2, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useCancelAiModel, useDeleteAiModel, useDownloadAiModel } from '../../ai/useAi';
import { formatBytes } from '../../lib/formatBytes';
import { Problem } from '../Problem';
import { downloadPercent, modelUse } from './modelUse';

const ICON_BUTTON =
  'hover:bg-surface grid size-10 shrink-0 place-items-center rounded-full disabled:opacity-60';

/** A download's bar, filled as the bytes arrive. */
export const DownloadBar: FC<{ model: AiModel }> = ({ model }) => {
  const { t } = useTranslation();
  return (
    <progress
      aria-label={t('ai.downloading', { name: model.name })}
      max={model.sizeBytes ?? 0}
      value={model.downloading ?? 0}
      className="bg-surface [&::-moz-progress-bar]:bg-primary [&::-webkit-progress-bar]:bg-surface [&::-webkit-progress-value]:bg-primary h-1.5 w-full appearance-none overflow-hidden rounded-full"
    />
  );
};

/** Stops a model's download at once: what came so far is deleted. */
export const CancelButton: FC<{ model: AiModel; className?: string }> = ({
  model,
  className = ICON_BUTTON,
}) => {
  const { t } = useTranslation();
  const cancel = useCancelAiModel();
  return (
    <button
      type="button"
      aria-label={t('ai.cancelDownload', { name: model.name })}
      title={t('ai.cancelDownload', { name: model.name })}
      disabled={cancel.isPending}
      onClick={() => cancel.mutate(model.id)}
      className={className}
    >
      <X aria-hidden size={18} />
    </button>
  );
};

/**
 * An on-device model: what it's for, its size and whether it's here, with Download, Cancel while
 * it downloads (with its progress), or Delete.
 */
export const ModelRow: FC<{ model: AiModel }> = ({ model }) => {
  const { t } = useTranslation();
  const download = useDownloadAiModel();
  const remove = useDeleteAiModel();
  const percent = downloadPercent(model);
  const state =
    percent !== null
      ? t('ai.onDevice.progress', { percent })
      : model.installed
        ? t('ai.installed')
        : null;
  const details = [
    t(`ai.onDevice.use.${modelUse(model)}`),
    formatBytes(model.sizeBytes ?? 0),
    state,
  ].filter(Boolean);

  return (
    <li aria-label={model.name} className="flex flex-col gap-2 px-1 py-3">
      <div className="flex items-center gap-2">
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-medium">{model.name}</span>
          <span className="text-muted text-sm">{details.join(' · ')}</span>
        </span>
        {percent !== null ? (
          <CancelButton model={model} />
        ) : model.installed ? (
          <button
            type="button"
            aria-label={t('ai.deleteModel', { name: model.name })}
            title={t('ai.deleteModel', { name: model.name })}
            disabled={remove.isPending}
            onClick={() => remove.mutate(model.id)}
            className={ICON_BUTTON}
          >
            <Trash2 aria-hidden size={18} />
          </button>
        ) : (
          <button
            type="button"
            aria-label={t('ai.downloadModel', { name: model.name })}
            title={t('ai.downloadModel', { name: model.name })}
            onClick={() => download.mutate(model.id)}
            className={ICON_BUTTON}
          >
            <Download aria-hidden size={18} />
          </button>
        )}
      </div>
      {percent !== null && <DownloadBar model={model} />}
      {download.error && (
        <Problem
          title={t('ai.downloadFailed', { name: model.name })}
          detail={download.error.message}
          onRetry={() => download.mutate(model.id)}
        />
      )}
      {remove.error && <Problem title={t('ai.deleteModelFailed')} detail={remove.error.message} />}
    </li>
  );
};

/**
 * Download for the model a feature runs (named by the row above), while it isn't here: its size,
 * then its progress with Cancel. Removing it is On-device models' work.
 */
export const ModelDownloadRow: FC<{ model: AiModel }> = ({ model }) => {
  const { t } = useTranslation();
  const download = useDownloadAiModel();
  const percent = downloadPercent(model);
  if (model.installed) return null;

  return (
    <li className="flex flex-col gap-2">
      {percent !== null ? (
        <div className="flex w-full items-center gap-4 px-1 py-3">
          <LoaderCircle aria-hidden size={22} className="text-primary shrink-0 animate-spin" />
          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="font-medium">{t('ai.onDevice.progress', { percent })}</span>
            <span className="text-muted text-sm">{formatBytes(model.sizeBytes ?? 0)}</span>
          </span>
          <CancelButton model={model} />
        </div>
      ) : (
        <button
          type="button"
          aria-label={t('ai.downloadModel', { name: model.name })}
          onClick={() => download.mutate(model.id)}
          className="hover:bg-surface flex w-full items-center gap-4 rounded-xl px-1 py-3 text-left"
        >
          <Download aria-hidden size={22} className="text-primary shrink-0" />
          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="font-medium">{t('ai.onDevice.download')}</span>
            <span className="text-muted text-sm">{formatBytes(model.sizeBytes ?? 0)}</span>
          </span>
        </button>
      )}
      {percent !== null && (
        <div className="px-1 pb-2">
          <DownloadBar model={model} />
        </div>
      )}
      {download.error && (
        <div className="px-1 pb-2">
          <Problem
            title={t('ai.downloadFailed', { name: model.name })}
            detail={download.error.message}
            onRetry={() => download.mutate(model.id)}
          />
        </div>
      )}
    </li>
  );
};
