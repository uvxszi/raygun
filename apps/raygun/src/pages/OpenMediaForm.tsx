import { commands } from '@raygun/bindings';
import { ListPlus, Play } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { toQueueItem } from './toQueueItem';

/** Plays an arbitrary stream URL or local file until the real library arrives (M07). */
export const OpenMediaForm: FC = () => {
  const { t } = useTranslation();
  const [location, setLocation] = useState('');
  const [referer, setReferer] = useState('');

  const submit = (playNow: boolean) => {
    const item = toQueueItem(location, referer);
    if (!item) {
      return;
    }
    void commands.playbackEnqueue([item], playNow);
    setLocation('');
  };

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    submit(true);
  };

  return (
    <form
      data-testid="open-media-form"
      onSubmit={onSubmit}
      className="bg-surface flex flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <input
        value={location}
        onChange={(event) => setLocation(event.target.value)}
        placeholder={t('library.openPlaceholder')}
        aria-label={t('library.openLocation')}
        inputMode="url"
        autoCapitalize="off"
        autoCorrect="off"
        spellCheck={false}
        className="bg-background placeholder:text-muted rounded-lg px-3 py-2 outline-none"
      />
      <input
        value={referer}
        onChange={(event) => setReferer(event.target.value)}
        placeholder={t('library.openRefererPlaceholder')}
        aria-label={t('library.openReferer')}
        inputMode="url"
        autoCapitalize="off"
        autoCorrect="off"
        spellCheck={false}
        className="bg-background placeholder:text-muted rounded-lg px-3 py-2 outline-none"
      />
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={!location.trim()}
          className="bg-primary text-background flex items-center gap-2 rounded-lg px-4 py-2 font-semibold disabled:opacity-40"
        >
          <Play aria-hidden size={16} />
          {t('library.openPlay')}
        </button>
        <button
          type="button"
          disabled={!location.trim()}
          onClick={() => submit(false)}
          className="bg-background flex items-center gap-2 rounded-lg px-4 py-2 disabled:opacity-40"
        >
          <ListPlus aria-hidden size={16} />
          {t('library.openQueue')}
        </button>
      </div>
    </form>
  );
};
