import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import type { FC } from 'react';

export const App: FC = () => {
  const appInfo = useQuery({ queryKey: ['appInfo'], queryFn: commands.appInfo });

  return (
    <main className="flex min-h-full flex-col items-center justify-center gap-4 p-6 pt-[max(1.5rem,env(safe-area-inset-top))]">
      <h1 className="text-primary text-4xl font-black tracking-tight">Raygun</h1>
      {appInfo.isPending && <p className="text-muted">Connecting to core…</p>}
      {appInfo.isError && (
        <p role="alert" className="text-primary">
          Core unavailable: {String(appInfo.error)}
        </p>
      )}
      {appInfo.data && (
        <dl
          data-testid="app-info"
          className="bg-surface grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 rounded-xl px-5 py-4 text-sm"
        >
          <dt className="text-muted">Version</dt>
          <dd>{appInfo.data.version}</dd>
          <dt className="text-muted">Platform</dt>
          <dd data-testid="platform">{appInfo.data.platform}</dd>
          <dt className="text-muted">Arch</dt>
          <dd>{appInfo.data.arch}</dd>
          <dt className="text-muted">Build</dt>
          <dd>{appInfo.data.debug ? 'debug' : 'release'}</dd>
        </dl>
      )}
    </main>
  );
};
