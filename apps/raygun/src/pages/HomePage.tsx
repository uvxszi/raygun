import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

export const HomePage: FC = () => {
  const { t } = useTranslation();
  return (
    <section data-testid="home-page" className="flex flex-col gap-2">
      <h1 className="text-primary text-3xl font-black tracking-tight lg:text-4xl">
        {t('home.title')}
      </h1>
      <p className="text-muted max-w-prose">{t('home.subtitle')}</p>
    </section>
  );
};
