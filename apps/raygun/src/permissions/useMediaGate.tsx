import { commands } from '@raygun/bindings';
import { useState } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { PermissionSheet } from './PermissionPrompt';

/**
 * Before a folder is added to Local Files: says why Raygun reads the phone's songs and videos and
 * asks, unless that's allowed (or not needed here). `proceed` runs after the answer either way;
 * without the permission the folder's comics and books still come in.
 */
export const useMediaGate = () => {
  const [waiting, setWaiting] = useState<(() => void) | null>(null);

  const guard = async (proceed: () => void) => {
    try {
      const states = await unwrapResult(commands.permissionsList());
      const media = states.find((permission) => permission.kind === 'media');
      if (media && media.state !== 'granted') {
        setWaiting(() => proceed);
        return;
      }
    } catch {
      // Adding the folder says what's wrong, if anything is.
    }
    proceed();
  };

  const sheet = waiting && (
    <PermissionSheet
      prompt={{ kind: 'media', reason: 'folders' }}
      onDone={() => {
        setWaiting(null);
        waiting();
      }}
    />
  );

  return { guard, sheet };
};
