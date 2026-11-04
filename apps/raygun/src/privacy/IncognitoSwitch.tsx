import { Check, HatGlasses } from 'lucide-react';
import { useId } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefSwitch } from '../settings/prefs';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { useIncognito } from './useIncognito';

/** The switch's knob, drawn as the settings rows draw theirs. */
const Knob: FC<{ on: boolean }> = ({ on }) => (
  <span
    aria-hidden
    className={`relative h-8 w-13 shrink-0 rounded-full border-2 transition-colors ${on ? 'bg-primary border-primary' : 'bg-surface border-muted/50'}`}
  >
    <span
      className={`absolute top-1/2 grid -translate-y-1/2 place-items-center rounded-full transition-all ${on ? 'bg-background left-[1.375rem] size-6' : 'bg-muted left-1.5 size-4'}`}
    >
      {on && <Check size={14} className="text-primary" strokeWidth={3} />}
    </span>
  </span>
);

/** Incognito's switch at the top of More (M47). */
export const IncognitoRow: FC = () => {
  const { t } = useTranslation();
  const incognito = useIncognito();
  const titleId = useId();
  return (
    <ul className="flex flex-col">
      <li>
        <button
          type="button"
          role="switch"
          aria-checked={incognito.on}
          aria-labelledby={titleId}
          disabled={incognito.busy}
          onClick={() => incognito.set(!incognito.on)}
          className="hover:bg-surface flex w-full items-center gap-4 rounded-xl px-2 py-3 text-left disabled:opacity-60"
        >
          <HatGlasses aria-hidden size={22} className="text-primary shrink-0" />
          <span className="flex min-w-0 flex-1 flex-col">
            <span id={titleId} className="font-medium">
              {t('incognito.title')}
            </span>
            <span className="text-muted text-xs">{t('incognito.hint')}</span>
          </span>
          <Knob on={incognito.on} />
        </button>
      </li>
    </ul>
  );
};

/** Settings → Data → Privacy's incognito rows (M47): the switch, and keeping it on across starts. */
export const IncognitoPrefs: FC = () => {
  const { t } = useTranslation();
  const incognito = useIncognito();
  const settings = useSettings();
  const update = useUpdateSetting();
  const privacy = settings.data?.privacy;
  return (
    <>
      <PrefSwitch
        title={t('incognito.title')}
        summary={t('incognito.hint')}
        checked={incognito.on}
        disabled={incognito.busy}
        onChange={incognito.set}
      />
      {privacy && (
        <PrefSwitch
          title={t('settings.privacy.keepIncognito')}
          summary={t('settings.privacy.keepIncognitoHint')}
          checked={privacy.keepIncognito ?? false}
          disabled={update.isPending}
          onChange={(keepIncognito) =>
            update.mutate({ key: 'privacy', value: { ...privacy, keepIncognito } })
          }
        />
      )}
    </>
  );
};
