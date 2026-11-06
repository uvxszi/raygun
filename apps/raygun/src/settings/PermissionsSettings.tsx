import type { AppPermission, PermissionKind } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import {
  BatteryCharging,
  Bell,
  Camera,
  Download,
  FolderOpen,
  HardDrive,
  Music,
  Wifi,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { canAsk, usePermissions, useRequestPermission } from '../permissions/usePermissions';
import { PrefGroup } from './prefs';

const ICONS: Record<PermissionKind, LucideIcon> = {
  notifications: Bell,
  battery: BatteryCharging,
  media: Music,
  folders: FolderOpen,
  allFiles: HardDrive,
  localNetwork: Wifi,
  installApps: Download,
  camera: Camera,
};

/** Permissions that are optional say what they add (M46.51). */
const ABOUT: Partial<Record<PermissionKind, string>> = {
  allFiles: 'permissions.about.allFiles',
};

/**
 * Settings → Permissions: each permission Raygun uses on this device, where it stands, and the
 * way to change it (Android's question, or its settings page where only that can).
 */
export const PermissionsSettings: FC = () => {
  const { t } = useTranslation();
  const permissions = usePermissions();
  const request = useRequestPermission();
  return (
    <PrefGroup>
      {permissions.isError && (
        <li role="alert" className="text-muted px-1 py-3 text-sm">
          {t('permissions.unreadable')}
        </li>
      )}
      {permissions.data?.map((permission) => (
        <PermissionRow
          key={permission.kind}
          permission={permission}
          busy={request.isPending && request.variables === permission.kind}
          onAsk={() => request.mutate(permission.kind)}
        />
      ))}
      {request.isError && (
        <li role="alert" className="text-muted px-1 py-2 text-sm">
          {t('permissions.askFailed')}
        </li>
      )}
    </PrefGroup>
  );
};

const PermissionRow: FC<{ permission: AppPermission; busy: boolean; onAsk: () => void }> = ({
  permission,
  busy,
  onAsk,
}) => {
  const { t } = useTranslation();
  const Icon = ICONS[permission.kind];
  const title = t(`permissions.kind.${permission.kind}`);
  const state =
    permission.kind === 'folders'
      ? t('permissions.folderCount', { count: permission.count ?? 0 })
      : t(`permissions.state.${permission.state}`);
  const granted = permission.state === 'granted';
  const about = ABOUT[permission.kind];
  return (
    <li
      className="flex items-center gap-4 rounded-xl px-1 py-3"
      data-testid={`permission-${permission.kind}`}
    >
      <Icon aria-hidden size={22} className="text-primary shrink-0" />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="font-medium">{title}</span>
        <span className="text-muted text-sm">{state}</span>
        {about && <span className="text-muted text-xs">{t(about)}</span>}
      </span>
      {permission.kind === 'folders' ? (
        <Link to="/local-files" className="bg-surface shrink-0 rounded-full px-4 py-1.5 text-sm">
          {t('permissions.manage')}
        </Link>
      ) : (
        !granted && (
          <button
            type="button"
            disabled={busy}
            onClick={onAsk}
            aria-label={`${canAsk(permission) ? t('permissions.allow') : t('permissions.openSettings')}: ${title}`}
            className="bg-primary text-background shrink-0 rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-60"
          >
            {canAsk(permission) ? t('permissions.allow') : t('permissions.openSettings')}
          </button>
        )
      )}
    </li>
  );
};
