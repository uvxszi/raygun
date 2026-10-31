import { commands } from '@raygun/bindings';
import type { StorageMode } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import type { FC, ReactNode } from 'react';

const STORAGE_MODE_LABELS: Record<StorageMode, string> = {
  portable: 'Portable (next to the app)',
  installed: 'System folders',
  mobile: 'App sandbox',
  override: 'Custom (RAYGUN_HOME)',
};

const InfoList: FC<{ testId: string; children: ReactNode }> = ({ testId, children }) => (
  <dl
    data-testid={testId}
    className="bg-surface grid w-full max-w-md grid-cols-[auto_1fr] gap-x-6 gap-y-1 rounded-xl px-5 py-4 text-sm"
  >
    {children}
  </dl>
);

export const App: FC = () => {
  const appInfo = useQuery({ queryKey: ['appInfo'], queryFn: commands.appInfo });
  const storageInfo = useQuery({ queryKey: ['storageInfo'], queryFn: commands.storageInfo });
  const error = appInfo.error ?? storageInfo.error;

  return (
    <main className="flex min-h-full flex-col items-center justify-center gap-4 p-6 pt-[max(1.5rem,env(safe-area-inset-top))]">
      <h1 className="text-primary text-4xl font-black tracking-tight">Raygun</h1>
      {appInfo.isPending && <p className="text-muted">Connecting to core…</p>}
      {error && (
        <p role="alert" className="text-primary">
          Core unavailable: {String(error)}
        </p>
      )}
      {appInfo.data && (
        <InfoList testId="app-info">
          <dt className="text-muted">Version</dt>
          <dd>{appInfo.data.version}</dd>
          <dt className="text-muted">Platform</dt>
          <dd data-testid="platform">{appInfo.data.platform}</dd>
          <dt className="text-muted">Arch</dt>
          <dd>{appInfo.data.arch}</dd>
          <dt className="text-muted">Build</dt>
          <dd>{appInfo.data.debug ? 'debug' : 'release'}</dd>
        </InfoList>
      )}
      {storageInfo.data && (
        <InfoList testId="storage-info">
          <dt className="text-muted">Storage</dt>
          <dd data-testid="storage-mode">{STORAGE_MODE_LABELS[storageInfo.data.mode]}</dd>
          <dt className="text-muted">Data folder</dt>
          <dd data-testid="storage-root" className="break-all">
            {storageInfo.data.root}
          </dd>
        </InfoList>
      )}
    </main>
  );
};
