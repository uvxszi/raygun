import { commands } from '@raygun/bindings';
import type { ImportPrefs } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { readerName, useImportReaders } from '../../collections/importReaders';
import { unwrapResult } from '../../lib/commandResult';
import { PrefChoice, PrefGroup, PrefText } from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';

const SECRET = ['import', 'spotifySecret'] as const;

/**
 * Settings → Import: which reader reads screenshots, and the person's own Spotify app (a Client
 * ID and secret from Spotify's developer dashboard) for playlists longer than the 100 songs
 * Spotify's embed shows.
 */
export const ImportSettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  const readers = useImportReaders();
  const client = useQueryClient();
  const secret = useQuery({ queryKey: SECRET, queryFn: () => commands.importSpotifySecretHint() });
  const setSecret = useMutation({
    mutationFn: (value: string) => unwrapResult(commands.importSetSpotifySecret(value)),
    onSettled: () => void client.invalidateQueries({ queryKey: SECRET }),
  });
  if (!data) return null;
  const reader = data.import?.reader ?? 'auto';
  const spotifyClientId = data.import?.spotifyClientId ?? '';
  const change = (next: Partial<ImportPrefs>) =>
    update.mutate({ key: 'import', value: { reader, spotifyClientId, ...next } });
  return (
    <>
      <PrefGroup title={t('import.settings.readers')} summary={t('import.settings.readersHint')}>
        <PrefChoice<string>
          title={t('import.reader')}
          value={reader}
          options={[
            { value: 'auto', label: t('import.settings.auto') },
            ...(readers.data ?? []).map((each) => ({
              value: each.id,
              label: readerName(each, t),
              hint: each.ready
                ? undefined
                : t('import.settings.downloadFirst', {
                    size: Math.round((each.sizeBytes ?? 0) / 1_000_000),
                  }),
            })),
          ]}
          onChange={(next) => change({ reader: next })}
        />
      </PrefGroup>
      <PrefGroup title={t('import.settings.spotify')} summary={t('import.settings.spotifyHint')}>
        <PrefText
          title={t('import.settings.clientId')}
          value={spotifyClientId}
          placeholder={t('import.settings.notSet')}
          check={(text) => (/^[A-Za-z0-9]*$/.test(text) ? null : t('import.settings.badId'))}
          onChange={(next) => change({ spotifyClientId: next })}
        />
        <PrefText
          title={t('import.settings.clientSecret')}
          value=""
          placeholder={
            secret.data
              ? t('import.settings.secretKept', { end: secret.data })
              : t('import.settings.notSet')
          }
          onChange={(value) => setSecret.mutate(value)}
        />
      </PrefGroup>
    </>
  );
};
