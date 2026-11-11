import type { AiModel, ReaderAiKind } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { commands } from '@raygun/bindings';
import {
  ChevronDown,
  ChevronUp,
  Download,
  Languages,
  LoaderCircle,
  Plus,
  Sparkles,
  Trash2,
  Wand2,
  X,
  ZoomIn,
} from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { Link } from '@tanstack/react-router';

import { useAiModels, useAiProviders, useCancelAiModel, useDownloadAiModel } from '../ai/useAi';
import { LANGUAGES, nativeName } from '../i18n/languages';
import { formatBytes } from '../lib/formatBytes';
import { downloadPercent } from '../settings/ai/modelUse';
import { OnOffSwitch } from '../shell/OnOffSwitch';
import { Sheet } from '../shell/Sheet';
import { KINDS_OF, modelsFor, newLayer, useReaderAiLayers } from './useReaderAi';
import type { Layer, ReaderMedium } from './useReaderAi';

const ICONS: Record<ReaderAiKind, typeof ZoomIn> = {
  upscale: ZoomIn,
  restore: Wand2,
  translate: Languages,
};

type ButtonProps = {
  medium: ReaderMedium;
  /** A layer is working on what's shown. */
  busy?: boolean;
  /** Why a layer couldn't do its work. */
  error?: string | null;
};

/**
 * The readers' AI button (bottom right, above the reader's own buttons): it opens the AI layers.
 * A dot shows while a layer is on, a ring while one works.
 */
export const ReaderAiButton: FC<ButtonProps> = ({ medium, busy = false, error = null }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState<HTMLElement | null>(null);
  const { layers } = useReaderAiLayers(medium);
  const on = layers.some((layer) => layer.enabled);
  return (
    <>
      <button
        type="button"
        aria-label={t('readerAi.button')}
        aria-haspopup="dialog"
        data-testid="reader-ai-button"
        onClick={(event) => {
          event.stopPropagation();
          setOpen(open ? null : event.currentTarget);
        }}
        className={`bg-surface/95 relative grid size-12 shrink-0 place-items-center rounded-full shadow-md backdrop-blur ${on ? 'text-primary' : ''}`}
      >
        {busy ? (
          <LoaderCircle aria-hidden size={20} className="animate-spin" />
        ) : (
          <Sparkles aria-hidden size={20} />
        )}
        {(on || error) && (
          <span
            aria-hidden
            className={`absolute top-2 right-2 size-2 rounded-full ${error ? 'bg-red-500' : 'bg-primary'}`}
          />
        )}
      </button>
      {open && (
        <ReaderAiSheet medium={medium} anchor={open} error={error} onClose={() => setOpen(null)} />
      )}
    </>
  );
};

const SELECT = 'bg-background min-w-0 flex-1 rounded-lg px-2 py-1.5 text-sm';

type SheetProps = {
  medium: ReaderMedium;
  /** What it stands beside on wide windows; Settings → AI opens it from a row. */
  anchor: HTMLElement | null;
  error: string | null;
  onClose: () => void;
};

/**
 * A reader's AI layers, the top one first: each can be turned on and off, moved, given its model
 * (and a translation its languages) or removed. They work from the bottom up, each on what the
 * one under it made.
 */
export const ReaderAiSheet: FC<SheetProps> = ({ medium, anchor, error, onClose }) => {
  const { t } = useTranslation();
  const { layers, save } = useReaderAiLayers(medium);
  const shown = [...layers].reverse();
  const change = (id: string, patch: Partial<Layer>) =>
    save(layers.map((layer) => (layer.id === id ? { ...layer, ...patch } : layer)));
  const move = (id: string, by: number) => {
    const at = layers.findIndex((layer) => layer.id === id);
    const to = at + by;
    if (at < 0 || to < 0 || to >= layers.length) return;
    const next = [...layers];
    [next[at], next[to]] = [next[to]!, next[at]!];
    save(next);
  };
  return (
    <Sheet title={t('readerAi.title')} anchor={anchor} popoverWidth={380} onClose={onClose}>
      <div className="flex flex-col gap-3" data-testid="reader-ai-sheet">
        <p className="text-muted px-1 text-sm">{t('readerAi.order')}</p>
        {error && (
          <p role="alert" className="rounded-lg bg-red-500/15 px-3 py-2 text-sm">
            {error}
          </p>
        )}
        {shown.length === 0 && <p className="text-muted px-1 text-sm">{t('readerAi.none')}</p>}
        <ol className="flex flex-col gap-2" aria-label={t('readerAi.layers')}>
          {shown.map((layer, place) => (
            <LayerRow
              key={layer.id}
              layer={layer}
              medium={medium}
              top={place === 0}
              bottom={place === shown.length - 1}
              onChange={(patch) => change(layer.id, patch)}
              onUp={() => move(layer.id, 1)}
              onDown={() => move(layer.id, -1)}
              onRemove={() => save(layers.filter((each) => each.id !== layer.id))}
            />
          ))}
        </ol>
        <div className="flex flex-wrap gap-2 px-1">
          {KINDS_OF[medium].map((kind) => {
            const Icon = ICONS[kind];
            return (
              <button
                key={kind}
                type="button"
                disabled={layers.length >= 8}
                // New layers go on top; a translation is into English until changed.
                onClick={() => save([...layers, newLayer(kind, 'en')])}
                className="bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm font-medium disabled:opacity-40"
              >
                <Plus aria-hidden size={14} />
                <Icon aria-hidden size={16} />
                {t(`readerAi.add.${kind}`)}
              </button>
            );
          })}
        </div>
        <Link
          to="/settings/ai"
          hash="models"
          onClick={onClose}
          className="text-primary px-1 pb-1 text-sm font-medium"
        >
          {t('readerAi.moreModels')}
        </Link>
      </div>
    </Sheet>
  );
};

type RowProps = {
  layer: Layer;
  medium: ReaderMedium;
  top: boolean;
  bottom: boolean;
  onChange: (patch: Partial<Layer>) => void;
  onUp: () => void;
  onDown: () => void;
  onRemove: () => void;
};

const LayerRow: FC<RowProps> = ({
  layer,
  medium,
  top,
  bottom,
  onChange,
  onUp,
  onDown,
  onRemove,
}) => {
  const { t } = useTranslation();
  const models = useAiModels().data ?? [];
  // A Whisper server only hears speech: it translates nothing.
  const providers = (useAiProviders().data ?? []).filter(
    (provider) => provider.kind !== 'whisperServer',
  );
  const Icon = ICONS[layer.kind];
  const name = t(`readerAi.kind.${layer.kind}`);
  const fitting = modelsFor(models, layer.kind);
  const installed = fitting.filter((model) => model.installed);
  const translate = layer.kind === 'translate';
  const languages =
    LANGUAGES.includes(layer.to) || !layer.to ? LANGUAGES : [layer.to, ...LANGUAGES];
  return (
    <li
      aria-label={name}
      className={`bg-background/60 flex flex-col gap-2 rounded-xl p-2 ${layer.enabled ? '' : 'opacity-70'}`}
    >
      <div className="flex items-center gap-1">
        <OnOffSwitch
          checked={layer.enabled}
          label={t('readerAi.turnOn', { name })}
          onChange={(enabled) => onChange({ enabled })}
        />
        <Icon aria-hidden size={18} className="shrink-0" />
        <span className="min-w-0 flex-1 truncate text-sm font-medium">{name}</span>
        <button
          type="button"
          aria-label={t('readerAi.moveUp', { name })}
          disabled={top}
          onClick={onUp}
          className="grid size-9 place-items-center rounded-full disabled:opacity-30"
        >
          <ChevronUp aria-hidden size={18} />
        </button>
        <button
          type="button"
          aria-label={t('readerAi.moveDown', { name })}
          disabled={bottom}
          onClick={onDown}
          className="grid size-9 place-items-center rounded-full disabled:opacity-30"
        >
          <ChevronDown aria-hidden size={18} />
        </button>
        <button
          type="button"
          aria-label={t('readerAi.remove', { name })}
          onClick={onRemove}
          className="grid size-9 place-items-center rounded-full"
        >
          <Trash2 aria-hidden size={16} />
        </button>
      </div>
      <label className="flex items-center gap-2 px-1 text-sm">
        <span className="text-muted w-20 shrink-0 text-xs">{t('readerAi.model')}</span>
        <select
          value={layer.model}
          aria-label={t('readerAi.modelOf', { name })}
          onChange={(event) => onChange({ model: event.target.value })}
          className={SELECT}
        >
          <option value="">
            {translate ? t('readerAi.device') : t('readerAi.firstInstalled')}
          </option>
          {installed.map((model) => (
            <option key={model.id} value={model.id}>
              {model.name}
            </option>
          ))}
          {translate &&
            providers.map((provider) => (
              <option key={provider.id} value={`ai:${provider.id}`}>
                {provider.name}
              </option>
            ))}
        </select>
      </label>
      {translate && (
        <div className="flex items-center gap-2 px-1 text-sm">
          <span className="text-muted w-20 shrink-0 text-xs">{t('readerAi.languages')}</span>
          <select
            value={layer.from}
            aria-label={t('readerAi.from')}
            onChange={(event) => onChange({ from: event.target.value })}
            className={SELECT}
          >
            <option value="">{t('readerAi.detect')}</option>
            {LANGUAGES.map((tag) => (
              <option key={tag} value={tag}>
                {nativeName(tag)}
              </option>
            ))}
          </select>
          <span aria-hidden>→</span>
          <select
            value={layer.to}
            aria-label={t('readerAi.to')}
            onChange={(event) => onChange({ to: event.target.value })}
            className={SELECT}
          >
            {languages.map((tag) => (
              <option key={tag} value={tag}>
                {nativeName(tag)}
              </option>
            ))}
          </select>
        </div>
      )}
      {!translate && installed.length === 0 && fitting[0] && <GetModel model={fitting[0]} />}
      {translate && !layer.model && layer.to && <DeviceTranslatorNeeded to={layer.to} />}
      {translate && medium === 'comic' && <TextReaderNeeded />}
    </li>
  );
};

/** The models this device's translator needs for `to` and hasn't downloaded. */
const DeviceTranslatorNeeded: FC<{ to: string }> = ({ to }) => {
  const options = useQuery({
    queryKey: ['readerAi', 'device', to],
    queryFn: () => commands.videoSubtitlesTranslators(to),
    staleTime: 60_000,
  });
  const models = useAiModels().data ?? [];
  const device =
    options.data?.status === 'ok'
      ? options.data.data.find((option) => option.id === 'device')
      : undefined;
  const missing = (device?.models ?? [])
    .map((id) => models.find((model) => model.id === id))
    .filter((model): model is AiModel => model !== undefined && !model.installed);
  return (
    <>
      {missing.map((model) => (
        <GetModel key={model.id} model={model} />
      ))}
    </>
  );
};

/** A model to download for a layer that has none yet; while it downloads, how far and Cancel. */
const GetModel: FC<{ model: AiModel }> = ({ model }) => {
  const { t } = useTranslation();
  const download = useDownloadAiModel();
  const cancel = useCancelAiModel();
  const percent = downloadPercent(model);
  return (
    <div className="flex items-center gap-2 px-1 text-sm">
      <span className="text-muted min-w-0 flex-1 text-xs">
        {t('readerAi.needsModel', { name: model.name, size: formatBytes(model.sizeBytes ?? 0) })}
      </span>
      {percent !== null ? (
        <>
          <span role="status" className="text-muted flex shrink-0 items-center gap-1.5 text-xs">
            <LoaderCircle aria-hidden size={14} className="animate-spin" />
            {t('readerAi.downloading', { percent })}
          </span>
          <button
            type="button"
            aria-label={t('ai.cancelDownload', { name: model.name })}
            disabled={cancel.isPending}
            onClick={() => cancel.mutate(model.id)}
            className="bg-background flex shrink-0 items-center gap-1 rounded-full px-3 py-1 text-xs font-semibold disabled:opacity-60"
          >
            <X aria-hidden size={14} />
            {t('readerAi.cancel')}
          </button>
        </>
      ) : (
        <button
          type="button"
          onClick={() => download.mutate(model.id)}
          className="bg-primary text-background flex shrink-0 items-center gap-1.5 rounded-full px-3 py-1 text-xs font-semibold"
        >
          <Download aria-hidden size={14} />
          {t('readerAi.download')}
        </button>
      )}
      {download.error && (
        <span role="alert" className="text-xs text-red-500">
          {download.error.message}
        </span>
      )}
    </div>
  );
};

/** Comic translation reads the pages' words: on Windows by itself, elsewhere with a model. */
const TextReaderNeeded: FC = () => {
  const support = useQuery({
    queryKey: ['readerAi', 'support'],
    queryFn: () => commands.readerAiSupport(),
    staleTime: Infinity,
  });
  const models = useAiModels().data ?? [];
  if (!support.data || support.data.textReaderBuiltIn) return null;
  const reader = models.find((model) => model.id === support.data.textReaderModel);
  if (!reader || reader.installed) return null;
  return <GetModel model={reader} />;
};
