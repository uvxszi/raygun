import type { AiModel, AiProvider, AiProviderKind } from '@raygun/bindings';
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
import { PageHeader } from './PageHeader';

const CARD = 'bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm';
const INPUT = 'bg-background min-w-0 rounded-lg px-3 py-2 outline-none';
const KINDS: AiProviderKind[] = ['claude', 'openAiCompatible', 'deepl'];

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
            className={INPUT}
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
