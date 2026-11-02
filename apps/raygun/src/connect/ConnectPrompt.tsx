import { commands, events } from '@raygun/bindings';
import type { ConnectRequest } from '@raygun/bindings';
import { MonitorSpeaker } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';

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
          <MonitorSpeaker aria-hidden size={28} className="text-primary shrink-0" />
          <p className="font-semibold">{t('connect.ask.question', { name: request.name })}</p>
        </div>
        <p className="text-muted text-sm">{t('connect.ask.explain', { name: request.name })}</p>
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={() => answer(false)}
            className="bg-background rounded-full px-4 py-2 text-sm font-semibold"
          >
            {t('connect.ask.deny')}
          </button>
          <button
            type="button"
            onClick={() => answer(true)}
            className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold"
          >
            {t('connect.ask.allow')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};
