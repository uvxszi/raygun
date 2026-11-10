import type { AiProvider, AiProviderKind } from '@raygun/bindings';
import { Languages, LoaderCircle, Pencil, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useAiProviders,
  useDeleteAiProvider,
  useSaveAiProvider,
  useTestAiProvider,
} from '../../ai/useAi';
import { Sheet } from '../../shell/Sheet';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../../shell/TextField';
import { ConfirmSheet } from '../ConfirmSheet';
import { PrefAction, PrefBlock, PrefGroup } from '../prefs';
import { Problem } from '../Problem';

const SELECT = 'bg-background min-w-0 rounded-xl px-4 py-3 outline-none';
const ICON_BUTTON =
  'hover:bg-surface grid size-10 shrink-0 place-items-center rounded-full disabled:opacity-60';
const KINDS: AiProviderKind[] = ['claude', 'openAiCompatible', 'deepl'];

/** A provider's service, name, key, address and model, in a sheet; `kind` starts a new one. */
export const ProviderForm: FC<{
  provider?: AiProvider;
  kind?: AiProviderKind;
  onDone: () => void;
}> = ({ provider, kind: startKind = 'claude', onDone }) => {
  const { t } = useTranslation();
  const save = useSaveAiProvider();
  const [kind, setKind] = useState<AiProviderKind>(provider?.kind ?? startKind);
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
    <Sheet title={provider ? provider.name : t('ai.providers.add')} onClose={onDone}>
      <form
        onSubmit={submit}
        aria-label={t('ai.providers.form')}
        className="flex flex-col gap-3 text-sm"
      >
        <label className="flex flex-col gap-1">
          <span className="text-muted text-xs">{t('ai.kindLabel')}</span>
          <select
            value={kind}
            onChange={(event) => setKind(event.target.value as AiProviderKind)}
            className={SELECT}
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
            className={SHEET_FIELD}
            inputClassName={SHEET_INPUT}
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
            className={SHEET_FIELD}
            inputClassName={SHEET_INPUT}
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
            type="url"
            value={baseUrl}
            onChange={setBaseUrl}
            placeholder={t(`ai.basePlaceholder.${kind}`)}
            autoCapitalize="off"
            className={SHEET_FIELD}
            inputClassName={SHEET_INPUT}
          />
        </label>
        {kind !== 'deepl' && (
          <label className="flex flex-col gap-1">
            <span className="text-muted text-xs">{t('ai.model')}</span>
            <TextField
              value={model}
              onChange={setModel}
              placeholder={t(`ai.modelPlaceholder.${kind}`)}
              autoCapitalize="off"
              className={SHEET_FIELD}
              inputClassName={SHEET_INPUT}
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

/** A provider: its service, model and key hint, with Test, Edit and Delete. */
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
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
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
          aria-haspopup="dialog"
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

/**
 * Online providers: Claude, OpenAI-compatible services and DeepL with their keys (kept in the
 * encrypted secrets store), each set up in a sheet.
 */
export const ProvidersSection: FC = () => {
  const { t } = useTranslation();
  const providers = useAiProviders();
  const [adding, setAdding] = useState(false);

  return (
    <>
      <PrefGroup id="providers" title={t('ai.providers.title')} summary={t('ai.providers.about')}>
        {providers.data?.length === 0 && (
          <li className="text-muted px-1 py-2 text-sm">{t('ai.providers.none')}</li>
        )}
        {providers.data?.map((provider) => (
          <ProviderRow key={provider.id} provider={provider} />
        ))}
        <PrefAction
          icon={Plus}
          title={t('ai.providers.add')}
          dialog
          onClick={() => setAdding(true)}
        />
        {providers.error && (
          <PrefBlock>
            <Problem title={t('ai.providers.listFailed')} detail={providers.error.message} />
          </PrefBlock>
        )}
      </PrefGroup>
      {adding && <ProviderForm onDone={() => setAdding(false)} />}
    </>
  );
};
