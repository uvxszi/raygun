import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider, createHashHistory } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import './i18n';
import { createAppRouter } from './router';
import './styles.css';

const queryClient = new QueryClient();
// Hash history: the app is served from a custom protocol where no server resolves deep paths,
// so routes live in the URL fragment.
const router = createAppRouter(createHashHistory());

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
);
