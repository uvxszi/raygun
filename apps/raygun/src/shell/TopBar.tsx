import { Link, useCanGoBack, useRouter } from '@tanstack/react-router';
import { ArrowLeft, X } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { IncognitoBadge } from '../privacy/IncognitoBadge';

type TopBarProps = {
  title: string;
  subtitle?: string | null;
  /** Shows a back arrow; goes back, or to `fallback` when the app was opened on this screen. */
  back?: boolean;
  fallback?: string;
  /** Icon actions on the right (`TopBarAction`, `OverflowMenu`). */
  actions?: ReactNode;
  /** The title is context, not the page's heading (title screens, whose header holds the name). */
  plainTitle?: boolean;
  /** Search mode (Mihon's): the title becomes a search field until `onClose`. */
  search?: TopBarSearch | null;
};

export type TopBarSearch = {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  onClose: () => void;
};

/**
 * A screen's top app bar (Mihon's): a large title on tab screens, a back arrow and a smaller
 * title on inner screens, icon actions on the right. Stays at the top while the page scrolls.
 */
export const TopBar: FC<TopBarProps> = ({
  title,
  subtitle,
  back,
  fallback = '/',
  actions,
  search,
  plainTitle,
}) => {
  const Title = plainTitle ? 'p' : 'h1';
  const { t } = useTranslation();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const goBack = () =>
    canGoBack ? router.history.back() : void router.navigate({ to: fallback, replace: true });

  return (
    <header
      data-testid="top-bar"
      className="bg-background sticky top-0 z-30 -mx-4 mb-1 flex min-h-14 items-center gap-1 px-2 lg:-mx-10 lg:px-6"
    >
      {search ? (
        <>
          <button
            type="button"
            aria-label={t('nav.closeSearch')}
            onClick={search.onClose}
            className="hover:bg-surface grid size-11 shrink-0 place-items-center rounded-full"
          >
            <ArrowLeft aria-hidden size={22} />
          </button>
          <input
            type="search"
            autoFocus
            value={search.value}
            onChange={(event) => search.onChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Escape') search.onClose();
            }}
            placeholder={search.placeholder}
            aria-label={search.placeholder}
            className="placeholder:text-muted min-w-0 flex-1 bg-transparent px-1 text-lg outline-none"
          />
          {search.value && (
            <button
              type="button"
              aria-label={t('nav.clearSearch')}
              onClick={() => search.onChange('')}
              className="hover:bg-surface grid size-11 shrink-0 place-items-center rounded-full"
            >
              <X aria-hidden size={20} />
            </button>
          )}
        </>
      ) : (
        <>
          {back && (
            <button
              type="button"
              aria-label={t('nav.back')}
              onClick={goBack}
              className="hover:bg-surface grid size-11 shrink-0 place-items-center rounded-full"
            >
              <ArrowLeft aria-hidden size={22} />
            </button>
          )}
          <div className={`flex min-w-0 flex-1 flex-col ${back ? 'px-1' : 'px-2'}`}>
            <Title
              className={
                back
                  ? 'truncate text-lg font-semibold'
                  : 'truncate text-2xl font-semibold tracking-tight'
              }
            >
              {title}
            </Title>
            {subtitle && <p className="text-muted truncate text-sm">{subtitle}</p>}
          </div>
        </>
      )}
      {/* Incognito (M47) shows on every screen while it's on; the desktop bar has its own. */}
      <IncognitoBadge phoneOnly className="mx-1" />
      {actions && <div className="flex shrink-0 items-center">{actions}</div>}
    </header>
  );
};

type TopBarActionProps = {
  label: string;
  icon: LucideIcon;
  onClick?: () => void;
  /** A link instead of a button. */
  to?: string;
  search?: Record<string, unknown>;
  pressed?: boolean;
  disabled?: boolean;
};

const ACTION_CLASS =
  'hover:bg-surface grid size-11 place-items-center rounded-full disabled:opacity-50';

/** An icon button in the top bar. */
export const TopBarAction: FC<TopBarActionProps> = ({
  label,
  icon: Icon,
  onClick,
  to,
  search,
  pressed,
  disabled,
}) =>
  to ? (
    <Link to={to} search={search} aria-label={label} className={ACTION_CLASS}>
      <Icon aria-hidden size={22} />
    </Link>
  ) : (
    <button
      type="button"
      aria-label={label}
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onClick}
      className={`${ACTION_CLASS} ${pressed ? 'text-primary' : ''}`}
    >
      <Icon aria-hidden size={22} />
    </button>
  );
