import { commands } from '@raygun/bindings';
import { useState } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { LocalNetworkSheet } from './LocalNetworkSheet';
import type { LocalNetworkReason } from './LocalNetworkSheet';

/**
 * Guards an action that reaches the local network (Android 17). `guard(address, proceed)` runs
 * `proceed` at once when no permission is needed (another platform, a public or loopback address,
 * or already allowed); otherwise `sheet` explains and asks first, and `proceed` runs once granted.
 * `address` is the addon's, or `null` for the LAN remote.
 */
export const useLocalNetworkGate = (reason: LocalNetworkReason) => {
  const [checking, setChecking] = useState(false);
  const [waiting, setWaiting] = useState<{ proceed: () => void } | null>(null);

  const guard = async (address: string | null, proceed: () => void) => {
    setChecking(true);
    try {
      const access = await unwrapResult(commands.localNetworkAccess(address));
      if (access.needed && !access.granted) {
        setWaiting({ proceed });
        return;
      }
    } catch {
      // The action itself then says what's wrong (an address that isn't one, say).
    } finally {
      setChecking(false);
    }
    proceed();
  };

  const sheet = waiting && (
    <LocalNetworkSheet
      reason={reason}
      onGranted={() => {
        setWaiting(null);
        waiting.proceed();
      }}
      onClose={() => setWaiting(null)}
    />
  );

  return { guard, checking, sheet };
};
