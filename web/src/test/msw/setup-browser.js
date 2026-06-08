import { afterAll, afterEach, beforeAll } from 'vitest';
import { worker } from './worker.js';

beforeAll(() => worker.start({ onUnhandledRequest: 'bypass', quiet: true }));
afterEach(() => worker.resetHandlers());
afterAll(() => worker.stop());
