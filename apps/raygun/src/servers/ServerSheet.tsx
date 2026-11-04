import type { ServerInfo, ServerKind } from '@raygun/bindings';
import { LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useLocalNetworkGate } from '../network/useLocalNetworkGate';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { SERVER_KINDS, TAKES_API_KEY, useAddServer } from './useServers';

/** Where each kind's address usually is, as the address field's hint. */
const ADDRESS_HINT: Record<ServerKind, string> = {
  jellyfin: 'jellyfin.example.org:8096',
  emby: 'emby.example.org:8096',
  subsonic: 'music.example.org:4533',
  audiobookshelf: 'books.example.org:13378',
  opds: 'example.org/opds',
};

/**
 * Connects a server: what kind it is, its address, and the sign-in (a password, or an API key
 * where the server has them). Signing in again to a connected one starts from its address and
 * user. A server on the home network asks for Android's local network access first.
 */
export const ServerSheet: FC<{ server?: ServerInfo; onClose: () => void }> = ({
  server,
  onClose,
}) => {
  const { t } = useTranslation();
  const add = useAddServer();
  const gate = useLocalNetworkGate('server');
  const [kind, setKind] = useState<ServerKind>(server?.kind ?? 'jellyfin');
  const [address, setAddress] = useState(server?.address ?? '');
  const [username, setUsername] = useState(server?.username ?? '');
  const [secret, setSecret] = useState('');
  const [apiKey, setApiKey] = useState(false);
  const keyed = apiKey && TAKES_API_KEY.has(kind);
  // A catalogue may be open to anyone: its sign-in is optional.
  const needsUser = kind !== 'opds' && !keyed;
  const busy = add.isPending || gate.checking;
  const ready = address.trim() !== '' && (!needsUser || username.trim() !== '') && !busy;

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (!ready) return;
    const input = {
      kind,
      address: address.trim(),
      username: keyed ? '' : username.trim(),
      secret,
      apiKey: keyed,
    };
    void gate.guard(input.address, () => add.mutate(input, { onSuccess: onClose }));
  };

  const chip = (on: boolean) =>
    `shrink-0 rounded-full px-3 py-1.5 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-background'}`;

  return (
    <Sheet title={server ? t('servers.signInAgain') : t('servers.add')} onClose={onClose}>
      <form onSubmit={onSubmit} className="flex flex-col gap-3 text-sm">
        <div role="radiogroup" aria-label={t('servers.kind')} className="flex flex-wrap gap-2">
          {SERVER_KINDS.map((option) => (
            <button
              key={option}
              type="button"
              role="radio"
              aria-checked={kind === option}
              disabled={Boolean(server)}
              onClick={() => setKind(option)}
              className={chip(kind === option)}
            >
              {t(`servers.kinds.${option}`)}
            </button>
          ))}
        </div>
        <TextField
          type="text"
          inputMode="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          autoFocus={!server}
          value={address}
          onChange={setAddress}
          placeholder={ADDRESS_HINT[kind]}
          aria-label={t('servers.address')}
          className={SHEET_FIELD}
          inputClassName={SHEET_INPUT}
        />
        {TAKES_API_KEY.has(kind) && (
          <label className="flex items-center gap-2 px-1">
            <input
              type="checkbox"
              checked={apiKey}
              onChange={(event) => setApiKey(event.target.checked)}
              className="accent-primary size-4"
            />
            {t('servers.useApiKey')}
          </label>
        )}
        {!keyed && (
          <TextField
            type="text"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            autoComplete="username"
            value={username}
            onChange={setUsername}
            placeholder={kind === 'opds' ? t('servers.usernameOptional') : t('servers.username')}
            aria-label={t('servers.username')}
            className={SHEET_FIELD}
            inputClassName={SHEET_INPUT}
          />
        )}
        <TextField
          type="password"
          autoComplete={keyed ? 'off' : 'current-password'}
          autoFocus={Boolean(server)}
          value={secret}
          onChange={setSecret}
          placeholder={keyed ? t('servers.apiKey') : t('servers.password')}
          aria-label={keyed ? t('servers.apiKey') : t('servers.password')}
          className={SHEET_FIELD}
          inputClassName={SHEET_INPUT}
        />
        {add.error && (
          // Rust says what to fix: the address, the sign-in, or reaching the server.
          <SourceIssue compact>{add.error.message}</SourceIssue>
        )}
        <button
          type="submit"
          disabled={!ready}
          className="bg-primary text-background flex items-center gap-1.5 self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-60"
        >
          {busy && <LoaderCircle aria-hidden size={16} className="animate-spin" />}
          {add.isPending ? t('servers.connecting') : t('servers.connect')}
        </button>
      </form>
      {gate.sheet}
    </Sheet>
  );
};
