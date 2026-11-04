import type {
  AiModel,
  AiProvider,
  AiProviderKind,
  TranscriptionEngine,
  TranscriptionPrefs,
} from '@raygun/bindings';
import { Download, Languages, LoaderCircle, Pencil, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useAiModels,
  useAiProviders,
  useDeleteAiModel,
  useDeleteAiProvider,
  useDownloadAiModel,
  useSaveAiProvider,
  useTestAiProvider,
} from '../ai/useAi';
import { ConfirmSheet } from '../settings/ConfirmSheet';
import { McpCard } from '../settings/McpCard';
import { PrefAction, PrefGroup } from '../settings/prefs';
import { Problem, SettingsSaveIssue } from '../settings/Problem';
import { useFoundPref } from '../settings/useSettingsSearch';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { TopBar } from '../shell/TopBar';

const INPUT = 'bg-background min-w-0 rounded-lg px-3 py-2 outline-none';
const ICON_BUTTON =
  'hover:bg-surface grid size-10 shrink-0 place-items-center rounded-full disabled:opacity-60';
const KINDS: AiProviderKind[] = ['claude', 'openAiCompatible', 'deepl'];
const ENGINES: TranscriptionEngine[] = ['off', 'local', 'online'];

const megabytes = (bytes: number) => (bytes / 1024 / 1024).toFixed(1);

/** A translator's service, name, key, address and model, in a sheet. */
const ProviderForm: FC<{ provider?: AiProvider; onDone: () => void }> = ({ provider, onDone }) => {
  const { t } = useTranslation();
  const save = useSaveAiProvider();
  const [kind, setKind] = useState<AiProviderKind>(provider?.kind ?? 'claude');
  const [name, setName] = useState(provider?.name ?? '');
  const [key, setKey] = useState('');
  const [removeKey, setRemoveKey] = useState(false);
  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? '');
  const [model, setModel] = useState(provider?.model ?? '');

  const submit = (event: FormEvent) => {
    event.preventDefault();
    save.mutate(
      {
        id: provider?.id ?? null,
        kind,
        name: name.trim() || t(`ai.kind.${kind}`),
        baseUrl: baseUrl.trim() || null,
        model: kind === 'deepl' ? null : model.trim() || null,
        key: removeKey ? '' : key.trim() || null,
      },
      { onSuccess: onDone },
    );
  };

  return (
    <Sheet title={provider ? provider.name : t('ai.addTranslator')} onClose={onDone}>
      <form
        onSubmit={submit}
        aria-label={t('ai.translatorForm')}
        className="flex flex-col gap-3 text-sm"
      >
        <label className="flex flex-col gap-1">
          <span className="text-muted text-xs">{t('ai.kindLabel')}</span>
          <select
            value={kind}
            onChange={(event) => setKind(event.target.value as AiProviderKind)}
            className={INPUT}
          >
            {KINDS.map((option) => (
              <option key={option} value={option}>
                {t(`ai.kind.${option}`)}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-muted text-xs">{t('ai.name')}</span>
          <input
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={t(`ai.kind.${kind}`)}
            maxLength={60}
            className={INPUT}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-muted text-xs">
            {kind === 'openAiCompatible' ? t('ai.apiKeyOptional') : t('ai.apiKey')}
          </span>
          <input
            type="password"
            autoComplete="off"
            value={key}
            disabled={removeKey}
            onChange={(event) => setKey(event.target.value)}
            placeholder={
              provider?.keyHint ? t('ai.savedKey', { hint: provider.keyHint }) : t('ai.pasteKey')
            }
            className={INPUT}
          />
        </label>
        {provider?.hasKey && (
          <label className="flex items-center gap-2 text-xs">
            <input
              type="checkbox"
              checked={removeKey}
              onChange={(event) => setRemoveKey(event.target.checked)}
              className="accent-primary size-4"
            />
            {t('ai.removeKey')}
          </label>
        )}
        <label className="flex flex-col gap-1">
          <span className="text-muted text-xs">{t('ai.baseUrl')}</span>
          <input
            value={baseUrl}
            onChange={(event) => setBaseUrl(event.target.value)}
            placeholder={t(`ai.basePlaceholder.${kind}`)}
            inputMode="url"
            autoCapitalize="off"
            className={INPUT}
          />
        </label>
        {kind !== 'deepl' && (
          <label className="flex flex-col gap-1">
            <span className="text-muted text-xs">{t('ai.model')}</span>
            <input
              value={model}
              onChange={(event) => setModel(event.target.value)}
              placeholder={t(`ai.modelPlaceholder.${kind}`)}
              autoCapitalize="off"
              className={INPUT}
            />
          </label>
        )}
        {save.error && <Problem title={t('ai.saveFailed')} detail={save.error.message} />}
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onDone} className="rounded-full px-4 py-2">
            {t('ai.cancel')}
          </button>
          <button
            type="submit"
            disabled={save.isPending}
            className="bg-primary text-background rounded-full px-4 py-2 font-semibold disabled:opacity-60"
          >
            {t('ai.save')}
          </button>
        </div>
      </form>
    </Sheet>
  );
};

const ProviderRow: FC<{ provider: AiProvider }> = ({ provider }) => {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const test = useTestAiProvider();
  const remove = useDeleteAiProvider();
  const details = [
    t(`ai.kind.${provider.kind}`),
    provider.model,
    provider.keyHint ? t('ai.keyHint', { hint: provider.keyHint }) : t('ai.noKey'),
  ].filter(Boolean);

  return (
    <li className="flex flex-col gap-2 px-1 py-2" aria-label={provider.name}>
      <div className="flex items-center gap-1">
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-medium">{provider.name}</span>
          <span className="text-muted truncate text-sm">{details.join(' · ')}</span>
        </span>
        <button
          type="button"
          aria-label={t('ai.test')}
          title={t('ai.test')}
          disabled={test.isPending}
          onClick={() => test.mutate(provider.id)}
          className={ICON_BUTTON}
        >
          {test.isPending ? (
            <LoaderCircle aria-hidden size={18} className="animate-spin" />
          ) : (
            <Languages aria-hidden size={18} />
          )}
        </button>
        <button
          type="button"
          aria-label={t('ai.edit')}
          title={t('ai.edit')}
          onClick={() => setEditing(true)}
          className={ICON_BUTTON}
        >
          <Pencil aria-hidden size={18} />
        </button>
        <button
          type="button"
          aria-label={t('ai.delete')}
          title={t('ai.delete')}
          disabled={remove.isPending}
          onClick={() => setDeleting(true)}
          className={ICON_BUTTON}
        >
          <Trash2 aria-hidden size={18} />
        </button>
      </div>
      {test.data !== undefined && !test.isPending && (
        <p role="status" className="bg-surface rounded-lg px-3 py-2 text-sm">
          {t('ai.testResult', { text: test.data })}
        </p>
      )}
      {test.error && (
        <Problem
          title={t('ai.testFailed', { name: provider.name })}
          detail={test.error.message}
          onRetry={() => test.mutate(provider.id)}
        />
      )}
      {remove.error && <Problem title={t('ai.deleteFailed')} detail={remove.error.message} />}
      {editing && <ProviderForm provider={provider} onDone={() => setEditing(false)} />}
      {deleting && (
        <ConfirmSheet
          title={t('ai.deleteQuestion', { name: provider.name })}
          confirm={t('ai.deleteConfirm')}
          onConfirm={() => remove.mutate(provider.id)}
          onClose={() => setDeleting(false)}
        />
      )}
    </li>
  );
};

const ModelRow: FC<{ model: AiModel }> = ({ model }) => {
  const { t } = useTranslation();
  const download = useDownloadAiModel();
  const remove = useDeleteAiModel();
  const downloading = model.downloading ?? null;

  return (
    <li className="flex flex-col gap-2 px-1 py-2" aria-label={model.name}>
      <div className="flex items-start gap-2">
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-medium">{model.name}</span>
          <span className="text-muted text-sm">
            {[
              t('ai.megabytes', { size: megabytes(model.sizeBytes ?? 0) }),
              model.license,
              model.installed ? t('ai.installed') : null,
            ]
              .filter(Boolean)
              .join(' · ')}
          </span>
        </span>
        {model.installed ? (
          <button
            type="button"
            aria-label={t('ai.deleteModel', { name: model.name })}
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
            disabled={downloading !== null}
            onClick={() => download.mutate(model.id)}
            className={ICON_BUTTON}
          >
            {downloading !== null ? (
              <LoaderCircle aria-hidden size={18} className="animate-spin" />
            ) : (
              <Download aria-hidden size={18} />
            )}
          </button>
        )}
      </div>
      {downloading !== null && (
        <progress
          aria-label={t('ai.downloading', { name: model.name })}
          max={model.sizeBytes ?? 0}
          value={downloading}
          className="bg-background h-1.5 w-full appearance-none overflow-hidden rounded-full [&::-moz-progress-bar]:bg-primary [&::-webkit-progress-bar]:bg-background [&::-webkit-progress-value]:bg-primary"
        />
      )}
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
 * Captions for podcasts and audiobooks (M46.32): Whisper on this device (its model below), or an
 * OpenAI-compatible provider from the translators above. Players link here ("Set up
 * transcription"), so the section has an anchor.
 */
const TranscriptionSection: FC<{ providers: AiProvider[]; models: AiModel[] }> = ({
  providers,
  models,
}) => {
  const { t } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  const prefs = settings.data?.transcription;
  const [modelName, setModelName] = useState<string | null>(null);
  if (!prefs) return null;
  const save = (change: Partial<TranscriptionPrefs>) =>
    update.mutate({ key: 'transcription', value: { ...prefs, ...change } });
  const speech = models.filter((model) => model.kind === 'speech');
  const local = speech.find((model) => model.id === prefs.localModel) ?? speech[0];
  const compatible = providers.filter((provider) => provider.kind === 'openAiCompatible');

  return (
    <section
      id="transcription"
      data-testid="ai-transcription"
      aria-label={t('ai.transcription.title')}
      className="flex flex-col gap-3 pt-3"
    >
      <h2 className="text-muted px-1 text-xs font-semibold">{t('ai.transcription.title')}</h2>
      <div
        role="radiogroup"
        aria-label={t('ai.transcription.engine')}
        className="flex flex-wrap gap-2 px-1"
      >
        {ENGINES.map((engine) => (
          <button
            key={engine}
            type="button"
            role="radio"
            aria-checked={prefs.engine === engine}
            onClick={() => save({ engine })}
            className={`rounded-full px-4 py-1.5 text-sm font-medium ${
              prefs.engine === engine ? 'bg-primary text-background' : 'bg-surface'
            }`}
          >
            {t(`ai.transcription.engines.${engine}`)}
          </button>
        ))}
      </div>
      {prefs.engine === 'local' && local && (
        <ul>
          <ModelRow model={local} />
        </ul>
      )}
      {prefs.engine === 'online' && (
        <div className="flex flex-col gap-3 px-1 text-sm">
          {compatible.length === 0 ? (
            <p className="text-muted">{t('ai.transcription.noProvider')}</p>
          ) : (
            <label className="flex flex-col gap-1">
              <span className="text-muted text-xs">{t('ai.transcription.provider')}</span>
              <select
                value={prefs.providerId ?? ''}
                onChange={(event) => save({ providerId: event.target.value || null })}
                className={INPUT}
              >
                <option value="">{t('ai.transcription.pickProvider')}</option>
                {compatible.map((provider) => (
                  <option key={provider.id} value={provider.id}>
                    {provider.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="flex flex-col gap-1">
            <span className="text-muted text-xs">{t('ai.model')}</span>
            <input
              value={modelName ?? prefs.onlineModel}
              onChange={(event) => setModelName(event.target.value)}
              onBlur={() => {
                if (modelName !== null && modelName.trim() !== prefs.onlineModel) {
                  save({ onlineModel: modelName.trim() || 'whisper-1' });
                }
                setModelName(null);
              }}
              placeholder="whisper-1"
              autoCapitalize="off"
              className={INPUT}
            />
          </label>
          <p className="text-muted text-xs">{t('ai.transcription.onlineHint')}</p>
        </div>
      )}
    </section>
  );
};

/** Settings → AI: translators (with their keys), transcription, models, and AI assistants. */
export const AiPage: FC = () => {
  const { t } = useTranslation();
  const providers = useAiProviders();
  const models = useAiModels();
  const [adding, setAdding] = useState(false);
  useFoundPref();

  return (
    <section data-testid="ai-page" className="flex max-w-2xl flex-col">
      <TopBar back fallback="/settings" title={t('ai.title')} />
      <PrefGroup title={t('ai.translators')}>
        {providers.data && providers.data.length === 0 && (
          <li className="text-muted px-1 py-2 text-sm">{t('ai.noTranslators')}</li>
        )}
        {providers.data?.map((provider) => (
          <ProviderRow key={provider.id} provider={provider} />
        ))}
        <PrefAction icon={Plus} title={t('ai.addTranslator')} onClick={() => setAdding(true)} />
        {providers.error && (
          <li className="px-1 py-2">
            <Problem title={t('ai.listFailed')} detail={providers.error.message} />
          </li>
        )}
      </PrefGroup>
      <TranscriptionSection providers={providers.data ?? []} models={models.data ?? []} />
      <PrefGroup title={t('ai.models')}>
        {models.data?.map((model) => (
          <ModelRow key={model.id} model={model} />
        ))}
      </PrefGroup>
      <McpCard />
      <SettingsSaveIssue />
      {adding && <ProviderForm onDone={() => setAdding(false)} />}
    </section>
  );
};
