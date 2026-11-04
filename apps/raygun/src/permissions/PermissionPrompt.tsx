import { commands, events } from '@raygun/bindings';
import type { PermissionKind, PermissionPrompt as Prompt } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { BatteryCharging, Bell, LoaderCircle, Music } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { PERMISSIONS_KEY } from './usePermissions';

const ICONS: Partial<Record<PermissionKind, LucideIcon>> = {
  notifications: Bell,
  battery: BatteryCharging,
  media: Music,
};

type PermissionSheetProps = {
  prompt: Prompt;
  /** Called once the question is answered either way. */
  onDone: () => void;
};

/** Says why a feature needs a permission now, with Allow and Not now; both are remembered. */
export const PermissionSheet: FC<PermissionSheetProps> = ({ prompt, onDone }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);
  const Icon = ICONS[prompt.kind] ?? Bell;
  const answer = async (allow: boolean) => {
    setBusy(true);
    try {
      const states = await unwrapResult(commands.permissionPromptAnswer(prompt, allow));
      queryClient.setQueryData(PERMISSIONS_KEY, states);
    } catch {
      // Settings → Permissions still has it.
    } finally {
      setBusy(false);
      onDone();
    }
  };
  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 sm:items-center">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="permission-title"
        className="bg-surface flex w-full max-w-md flex-col gap-4 rounded-t-2xl px-5 pt-5 pb-[calc(1.25rem+env(safe-area-inset-bottom))] sm:rounded-2xl"
      >
        <div className="flex items-center gap-3">
          <span
            aria-hidden
            className="bg-background text-primary grid size-10 shrink-0 place-items-center rounded-full"
          >
            <Icon size={20} />
          </span>
          <h2 id="permission-title" className="text-lg font-semibold">
            {t(`permissions.ask.${prompt.reason}.title`)}
          </h2>
        </div>
        <p className="text-muted text-sm">{t(`permissions.ask.${prompt.reason}.why`)}</p>
        <div className="flex justify-end gap-2">
          <button
            type="button"
            disabled={busy}
            onClick={() => void answer(false)}
            className="bg-background rounded-full px-5 py-2 text-sm disabled:opacity-50"
          >
            {t('permissions.notNow')}
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => void answer(true)}
            className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-70"
          >
            {busy && <LoaderCircle aria-hidden size={16} className="animate-spin" />}
            {t('permissions.allow')}
          </button>
        </div>
      </div>
    </div>
  );
};

/**
 * Shows the questions Rust raises when a feature first needs a permission (the first playback
 * while Raygun is restricted, the first download), including one raised before this listened.
 */
export const PermissionPromptHost: FC = () => {
  const [prompt, setPrompt] = useState<Prompt | null>(null);
  useEffect(() => {
    let alive = true;
    void commands.permissionPromptPending().then((pending) => {
      if (alive && pending) setPrompt(pending);
    });
    const unlisten = events.permissionPromptEvent.listen(({ payload }) => setPrompt(payload));
    return () => {
      alive = false;
      void unlisten.then((stop) => stop());
    };
  }, []);
  if (!prompt) return null;
  return <PermissionSheet prompt={prompt} onDone={() => setPrompt(null)} />;
};
