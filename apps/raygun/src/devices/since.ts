import { useTranslation } from 'react-i18next';

/** "Synced 3 minutes ago", in the reader's language. */
export const useSince = () => {
  const { t, i18n } = useTranslation();
  const relative = new Intl.RelativeTimeFormat(i18n.language, { numeric: 'auto' });
  return (at: number | null | undefined) => {
    if (!at) return t('link.notSynced');
    const minutes = Math.round((at - Date.now()) / 60_000);
    const when =
      Math.abs(minutes) < 60
        ? relative.format(minutes, 'minute')
        : Math.abs(minutes) < 48 * 60
          ? relative.format(Math.round(minutes / 60), 'hour')
          : relative.format(Math.round(minutes / 1440), 'day');
    return t('link.synced', { when });
  };
};
