import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { AppProviders } from './providers';
import { createQueryClient } from './query-client';
import { createAppRouter } from './router';

import './index.css';

const container = document.getElementById('root');
if (container === null) throw new Error('index.html has no #root element');

const queryClient = createQueryClient();
const router = createAppRouter({ queryClient });

createRoot(container).render(
  <StrictMode>
    <AppProviders queryClient={queryClient} router={router} />
  </StrictMode>,
);
