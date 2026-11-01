import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels, useAiProviders } from '../ai/useAi';

/** Translators and models at a glance, linking to the AI page. */
export const AiCard: FC = () => {
  const { t } = useTranslation();
  const providers = useAiProviders();
  const models = useAiModels();
  const installed = models.data?.filter((model) => model.installed).length ?? 0;

  return (
    <Link
      to="/settings/ai"
      data-testid="ai-card"
      className="bg-surface flex items-center gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <span className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="text-muted">{t('ai.title')}</span>
        <span>{t('ai.summary', { count: providers.data?.length ?? 0, models: installed })}</span>
      </span>
      <span className="text-muted flex items-center gap-1">
        {t('ai.manage')}
        <ChevronRight aria-hidden size={18} />
      </span>
    </Link>
  );
};
