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
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { TextField } from '../shell/TextField';
import { PageHeader } from './PageHeader';

const CARD = 'bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm';
const INPUT = 'bg-background min-w-0 rounded-lg px-3 py-2 outline-none';
/** Text fields: the same box, with room for the clear button. */
const FIELD = 'bg-background rounded-lg pr-0.5 pl-3';
const FIELD_TEXT = 'py-2';
const KINDS: AiProviderKind[] = ['claude', 'openAiCompatible', 'deepl'];
const ENGINES: TranscriptionEngine[] = ['off', 'local', 'online'];

const ErrorText: FC<{ error: Error | null | undefined }> = ({ error }) =>
  error ? (
    <p role="alert" className="text-primary text-sm wrap-anywhere">
      {error.message}
    </p>
  ) : null;

const megabytes = (bytes: number) => (bytes / 1024 / 1024).toFixed(1);

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
        baseUrl: baseUrl || null,
        model: kind === 'deepl' ? null : model || null,
        key: removeKey ? '' : key || null,
      },
      { onSuccess: onDone },
    );
  };

  return (
    <form onSubmit={submit} aria-label={t('ai.translatorForm')} className="flex flex-col gap-2">
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
        <TextField
          value={name}
          onChange={setName}
          placeholder={t(`ai.kind.${kind}`)}
          maxLength={60}
          aria-label={t('ai.name')}
          className={FIELD}
          inputClassName={FIELD_TEXT}
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-muted text-xs">
          {kind === 'openAiCompatible' ? t('ai.apiKeyOptional') : t('ai.apiKey')}
        </span>
        <TextField
          type="password"
          autoComplete="off"
          value={key}
          disabled={removeKey}
          onChange={setKey}
          placeholder={
            provider?.keyHint ? t('ai.savedKey', { hint: provider.keyHint }) : t('ai.pasteKey')
          }
          aria-label={kind === 'openAiCompatible' ? t('ai.apiKeyOptional') : t('ai.apiKey')}
          className={FIELD}
          inputClassName={FIELD_TEXT}
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
        <TextField
          value={baseUrl}
          onChange={setBaseUrl}
          placeholder={t(`ai.basePlaceholder.${kind}`)}
          inputMode="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          aria-label={t('ai.baseUrl')}
          className={FIELD}
          inputClassName={FIELD_TEXT}
        />
      </label>
      {kind !== 'deepl' && (
        <label className="flex flex-col gap-1">
          <span className="text-muted text-xs">{t('ai.model')}</span>
          <TextField
            value={model}
            onChange={setModel}
            placeholder={t(`ai.modelPlaceholder.${kind}`)}
            aria-label={t('ai.model')}
            className={FIELD}
            inputClassName={FIELD_TEXT}
          />
        </label>
      )}
      <ErrorText error={save.error} />
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={save.isPending}
          className="bg-primary text-background rounded-full px-4 py-2 font-semibold disabled:opacity-60"
        >
          {t('ai.save')}
        </button>
        <button type="button" onClick={onDone} className="bg-background rounded-full px-4 py-2">
          {t('ai.cancel')}
        </button>
      </div>
    </form>
  );
};

const ProviderRow: FC<{ provider: AiProvider }> = ({ provider }) => {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const test = useTestAiProvider();
  const remove = useDeleteAiProvider();

  if (editing) return <ProviderForm provider={provider} onDone={() => setEditing(false)} />;
  const details = [
    t(`ai.kind.${provider.kind}`),
    provider.model,
    provider.keyHint ? t('ai.keyHint', { hint: provider.keyHint }) : t('ai.noKey'),
  ].filter(Boolean);

  return (
    <li className="flex flex-col gap-2" aria-label={provider.name}>
      <div className="flex items-center gap-2">
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="font-semibold">{provider.name}</span>
          <span className="text-muted text-xs">{details.join(' · ')}</span>
        </span>
        <button
          type="button"
          aria-label={t('ai.test')}
          disabled={test.isPending}
          onClick={() => test.mutate(provider.id)}
          className="bg-background grid size-9 place-items-center rounded-full disabled:opacity-60"
        >
          {test.isPending ? (
            <LoaderCircle aria-hidden size={16} className="animate-spin" />
          ) : (
            <Languages aria-hidden size={16} />
          )}
        </button>
        <button
          type="button"
          aria-label={t('ai.edit')}
          onClick={() => setEditing(true)}
          className="bg-background grid size-9 place-items-center rounded-full"
        >
          <Pencil aria-hidden size={16} />
        </button>
        <button
          type="button"
          aria-label={t('ai.delete')}
          disabled={remove.isPending}
          onClick={() => remove.mutate(provider.id)}
          className="bg-background grid size-9 place-items-center rounded-full disabled:opacity-60"
        >
          <Trash2 aria-hidden size={16} />
        </button>
      </div>
      {test.data !== undefined && (
        <p role="status" className="bg-background rounded-lg px-3 py-2">
          {t('ai.testResult', { text: test.data })}
        </p>
      )}
      <ErrorText error={test.error ?? remove.error} />
    </li>
  );
};

const ModelRow: FC<{ model: AiModel }> = ({ model }) => {
  const { t } = useTranslation();
  const download = useDownloadAiModel();
  const remove = useDeleteAiModel();
  const downloading = model.downloading ?? null;

  return (
    <li className="flex flex-col gap-2" aria-label={model.name}>
      <div className="flex items-start gap-2">
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-semibold">{model.name}</span>
          <span className="text-muted text-xs">{model.description}</span>
          <span className="text-muted text-xs">
            {t('ai.megabytes', { size: megabytes(model.sizeBytes ?? 0) })} ·{' '}
            <a href={model.licenseUrl} target="_blank" rel="noreferrer" className="underline">
              {model.license}
            </a>
            {model.installed && ` · ${t('ai.installed')}`}
          </span>
        </span>
        {model.installed ? (
          <button
            type="button"
            aria-label={t('ai.deleteModel', { name: model.name })}
            disabled={remove.isPending}
            onClick={() => remove.mutate(model.id)}
            className="bg-background grid size-9 place-items-center rounded-full disabled:opacity-60"
          >
            <Trash2 aria-hidden size={16} />
          </button>
        ) : (
          <button
            type="button"
            aria-label={t('ai.downloadModel', { name: model.name })}
            disabled={downloading !== null}
            onClick={() => download.mutate(model.id)}
            className="bg-background grid size-9 place-items-center rounded-full disabled:opacity-60"
          >
            {downloading !== null ? (
              <LoaderCircle aria-hidden size={16} className="animate-spin" />
            ) : (
              <Download aria-hidden size={16} />
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
      <ErrorText error={download.error ?? remove.error} />
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
      className={CARD}
      aria-labelledby="ai-transcription"
    >
      <h2 id="ai-transcription" className="text-muted">
        {t('ai.transcription.title')}
      </h2>
      <p className="text-muted text-xs">{t('ai.transcription.hint')}</p>
      <div
        role="radiogroup"
        aria-label={t('ai.transcription.engine')}
        className="flex flex-wrap gap-2"
      >
        {ENGINES.map((engine) => (
          <button
            key={engine}
            type="button"
            role="radio"
            aria-checked={prefs.engine === engine}
            onClick={() => save({ engine })}
            className={`rounded-full px-4 py-1.5 font-medium ${
              prefs.engine === engine ? 'bg-primary text-background' : 'bg-background'
            }`}
          >
            {t(`ai.transcription.engines.${engine}`)}
          </button>
        ))}
      </div>
      {prefs.engine === 'local' && local && (
        <>
          <p className="text-muted text-xs">
            {t(local.installed ? 'ai.transcription.localReady' : 'ai.transcription.localNeeds')}
          </p>
          <ul>
            <ModelRow model={local} />
          </ul>
        </>
      )}
      {prefs.engine === 'online' && (
        <>
          {compatible.length === 0 ? (
            <p className="text-muted text-xs">{t('ai.transcription.noProvider')}</p>
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
            <TextField
              value={modelName ?? prefs.onlineModel ?? ''}
              onChange={setModelName}
              onBlur={() => {
                if (modelName !== null && modelName.trim() !== prefs.onlineModel) {
                  save({ onlineModel: modelName.trim() || 'whisper-1' });
                }
                setModelName(null);
              }}
              placeholder="whisper-1"
              aria-label={t('ai.model')}
              className={FIELD}
              inputClassName={FIELD_TEXT}
            />
          </label>
          <p className="text-muted text-xs">{t('ai.transcription.onlineHint')}</p>
        </>
      )}
    </section>
  );
};

/** Settings → AI: translators (with their keys) and downloadable models. */
export const AiPage: FC = () => {
  const { t } = useTranslation();
  const providers = useAiProviders();
  const models = useAiModels();
  const [adding, setAdding] = useState(false);

  return (
    <section data-testid="ai-page" className="flex max-w-2xl flex-col gap-4">
      <PageHeader back fallback="/more" title={t('ai.title')} />
      <section className={CARD} aria-labelledby="ai-translators">
        <h2 id="ai-translators" className="text-muted">
          {t('ai.translators')}
        </h2>
        <p className="text-muted text-xs">{t('ai.translatorsHint')}</p>
        {providers.data && providers.data.length === 0 && !adding && <p>{t('ai.noTranslators')}</p>}
        <ul className="flex flex-col gap-4">
          {providers.data?.map((provider) => (
            <ProviderRow key={provider.id} provider={provider} />
          ))}
        </ul>
        {adding ? (
          <ProviderForm onDone={() => setAdding(false)} />
        ) : (
          <button
            type="button"
            onClick={() => setAdding(true)}
            className="bg-primary text-background flex w-fit items-center gap-2 rounded-full px-4 py-2 font-semibold"
          >
            <Plus aria-hidden size={16} />
            {t('ai.addTranslator')}
          </button>
        )}
        <ErrorText error={providers.error} />
      </section>
      <TranscriptionSection providers={providers.data ?? []} models={models.data ?? []} />
      <section className={CARD} aria-labelledby="ai-models">
        <h2 id="ai-models" className="text-muted">
          {t('ai.models')}
        </h2>
        <p className="text-muted text-xs">{t('ai.modelsHint')}</p>
        <ul className="flex flex-col gap-4">
          {models.data?.map((model) => (
            <ModelRow key={model.id} model={model} />
          ))}
        </ul>
      </section>
    </section>
  );
};
