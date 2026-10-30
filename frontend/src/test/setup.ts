// Vitest setup for the `dom` project: jest-dom matchers and a clean DOM between tests.
import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

// jsdom has no scrolling; the router's scroll restoration calls it on every navigation.
window.scrollTo = () => undefined;

afterEach(() => {
  cleanup();
});
