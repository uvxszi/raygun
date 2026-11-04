import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { showToast } from '../shell/toastStore';

/** Opens a link from release notes in the system's browser. */
export const useOpenNotesLink = () => {
  const { t } = useTranslation();
  return (url: string) => {
    void commands.appOpenLink(url).then((result) => {
      if (result.status === 'error') showToast(t('updater.linkFailed'));
    });
  };
};

/** The notes bundled with this version and the ones before it, newest first. */
export const useBundledNotes = () =>
  useQuery({
    queryKey: ['appReleaseNotes'],
    queryFn: () => commands.appReleaseNotes(),
    staleTime: Infinity,
  });

/** This version's notes when this is its first start after an update (once per version). */
export const usePendingNotes = () =>
  useQuery({
    queryKey: ['appWhatsNew'],
    queryFn: () => unwrapResult(commands.appWhatsNew()),
    staleTime: Infinity,
    retry: false,
  });
