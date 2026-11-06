import { commands, events } from '@raygun/bindings';
import type { ConnectRequest } from '@raygun/bindings';
import { MonitorSpeaker } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';

/** The prompt's answers: 48 px tall, sharing the width on a phone. */
const ANSWER =
  'flex min-h-12 flex-1 items-center justify-center rounded-full px-6 text-base font-semibold sm:flex-none';

/**
 * The question another device asks before it first plays here (M46.15): allowing it pairs the two,
 * so it plays here from then on without asking. Closes by itself once Rust stops waiting.
 */
export const ConnectPrompt: FC = () => {
  const { t } = useTranslation();
  const [request, setRequest] = useState<ConnectRequest | null>(null);

  useEffect(() => {
    const unlisten = events.connectRequestEvent.listen(({ payload }) =>
      setRequest((shown) => (payload.open ? payload : shown?.id === payload.id ? null : shown)),
    );
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  if (!request) return null;
  const answer = (allow: boolean) => {
    void commands.connectAnswer(request.id, allow);
    setRequest(null);
  };
  return (
    <Sheet title={t('connect.ask.title')} onClose={() => answer(false)}>
      <div data-testid="connect-prompt" className="flex flex-col gap-4 pb-2">
        <div className="flex items-center gap-3">
          <MonitorSpeaker aria-hidden size={32} className="text-primary shrink-0" />
          <p className="text-base font-semibold">
            {t('connect.ask.question', { name: request.name })}
          </p>
        </div>
        <p className="text-muted text-sm">{t('connect.ask.explain', { name: request.name })}</p>
        {/* Two big answers, side by side: easy to hit with a thumb, the right one allows. */}
        <div className="flex gap-3 sm:justify-end">
          <button type="button" onClick={() => answer(false)} className={`${ANSWER} bg-background`}>
            {t('connect.ask.deny')}
          </button>
          <button
            type="button"
            onClick={() => answer(true)}
            className={`${ANSWER} bg-primary text-background`}
          >
            {t('connect.ask.allow')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};
