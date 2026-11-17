import type { ChipId, ChipInfo } from '@raygun/bindings';
import { Captions, Cpu, Shuffle, Waves } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { formatBytes } from '../lib/formatBytes';
import { OnOffSwitch } from '../shell/OnOffSwitch';
import { useChipActions, useChips } from './useChips';

const ICONS: Record<ChipId, LucideIcon> = {
  smartShuffle: Shuffle,
  smoothMotion: Waves,
  smartSubtitles: Captions,
};

const pill =
  'rounded-full px-3 py-1.5 text-sm font-medium disabled:opacity-60 bg-surface hover:bg-muted/20';

type RowProps = {
  chip: ChipInfo;
  actions: ReturnType<typeof useChipActions>;
};

/** One chip: what it does and its size, then Install (with progress and Cancel) or its switch. */
const ChipRow: FC<RowProps> = ({ chip, actions }) => {
  const { t } = useTranslation();
  const [removing, setRemoving] = useState(false);
  const Icon = ICONS[chip.id] ?? Cpu;
  const name = t(`smartChips.chips.${chip.id}.name`, { defaultValue: chip.name });
  const description = t(`smartChips.chips.${chip.id}.description`, {
    defaultValue: chip.description,
  });
  const progress = chip.progress;
  const downloading = progress.kind === 'downloading';
  const total = progress.kind === 'downloading' ? (progress.total ?? 0) : 0;
  const share = progress.kind === 'downloading' && total > 0 ? (progress.received ?? 0) / total : 0;
  return (
    <li data-testid={`chip-${chip.id}`} className="flex flex-col gap-2 px-3 py-3">
      <div className="flex items-start gap-3">
        <span className="bg-primary/15 text-primary grid size-10 shrink-0 place-items-center rounded-full">
          <Icon aria-hidden size={22} />
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-semibold">{name}</span>
          <span className="text-muted text-sm">{description}</span>
          <span className="text-muted text-xs">
            {t('smartChips.size', { size: formatBytes(chip.size ?? 0) })}
          </span>
        </span>
        {chip.installed && !downloading && (
          <OnOffSwitch
            checked={chip.on}
            label={t('smartChips.turnOn', { name })}
            disabled={actions.setEnabled.isPending}
            onChange={(enabled) => actions.setEnabled.mutate({ id: chip.id, enabled })}
          />
        )}
      </div>
      {downloading && (
        <div className="flex items-center gap-2 pl-13">
          <div
            role="progressbar"
            aria-label={t('smartChips.downloading', { name })}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(share * 100)}
            className="bg-surface h-1.5 flex-1 overflow-hidden rounded-full"
          >
            <div className="bg-primary h-full" style={{ width: `${share * 100}%` }} />
          </div>
          <button type="button" className={pill} onClick={() => actions.cancel.mutate(chip.id)}>
            {t('common.cancel')}
          </button>
        </div>
      )}
      {progress.kind === 'failed' && (
        <p role="alert" className="text-muted pl-13 text-sm">
          {t('smartChips.failed', { message: progress.message })}
        </p>
      )}
      {!downloading && (
        <div className="flex flex-wrap gap-2 pl-13">
          {!chip.installed && (
            <button type="button" className={pill} onClick={() => actions.install.mutate(chip.id)}>
              {t('smartChips.install')}
            </button>
          )}
          {chip.installed && chip.outdated && (
            <button type="button" className={pill} onClick={() => actions.install.mutate(chip.id)}>
              {t('smartChips.update')}
            </button>
          )}
          {chip.installed && !removing && (
            <button type="button" className={pill} onClick={() => setRemoving(true)}>
              {t('smartChips.remove')}
            </button>
          )}
          {chip.installed && removing && (
            <>
              <button
                type="button"
                className={pill}
                onClick={() => {
                  setRemoving(false);
                  actions.remove.mutate(chip.id);
                }}
              >
                {t('smartChips.removeConfirm', { name })}
              </button>
              <button type="button" className={pill} onClick={() => setRemoving(false)}>
                {t('common.cancel')}
              </button>
            </>
          )}
        </div>
      )}
    </li>
  );
};

/** Every Smart Chip, installed or not: the Smart Chips sheet and Settings → Smart Chips. */
export const ChipList: FC = () => {
  const { t } = useTranslation();
  const chips = useChips();
  const actions = useChipActions();
  if (chips.isPending) {
    return <p className="text-muted px-3 py-2 text-sm">{t('smartChips.loading')}</p>;
  }
  return (
    <ul aria-label={t('smartChips.label')} className="divide-surface flex flex-col divide-y">
      {(chips.data ?? []).map((chip) => (
        <ChipRow key={chip.id} chip={chip} actions={actions} />
      ))}
    </ul>
  );
};
