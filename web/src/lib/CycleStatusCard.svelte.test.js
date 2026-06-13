import { vi, describe, it, expect, beforeEach, afterEach } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { flushSync } from 'svelte';
import CycleStatusCard from './CycleStatusCard.svelte';

const mockHealth = vi.hoisted(() => {
  const defaults = {
    updating: false,
    schedule_mode: '',
    schedule_interval_secs: null,
    schedule_cron: null,
    next_run_at: null,
    cycle_progress: null,
  };
  let _state = { ...defaults };
  const _subs = new Set();
  return {
    subscribe(fn) {
      _subs.add(fn);
      fn(_state);
      return () => _subs.delete(fn);
    },
    set(v) {
      _state = v;
      _subs.forEach((fn) => fn(_state));
    },
    reset() {
      _state = { ...defaults };
    },
  };
});

vi.mock('../stores/health.js', () => ({ health: mockHealth }));

describe('CycleStatusCard', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockHealth.reset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('idle state', () => {
    it('renders run_once schedule label', async () => {
      mockHealth.set({ schedule_mode: 'run_once', next_run_at: null });
      const screen = render(CycleStatusCard, { watchedCount: 3 });
      await expect.element(screen.container.querySelector('.type-body-sm')).toHaveTextContent(
        '(run once only)',
      );
    });

    it('renders interval label in hours', async () => {
      mockHealth.set({ schedule_mode: 'interval', schedule_interval_secs: 7200, next_run_at: null });
      const screen = render(CycleStatusCard, { watchedCount: 3 });
      await expect.element(screen.container.querySelector('.type-body-sm')).toHaveTextContent(
        'every 2h',
      );
    });

    it('renders interval label in minutes', async () => {
      mockHealth.set({ schedule_mode: 'interval', schedule_interval_secs: 300, next_run_at: null });
      const screen = render(CycleStatusCard, { watchedCount: 3 });
      await expect.element(screen.container.querySelector('.type-body-sm')).toHaveTextContent(
        'every 5m',
      );
    });

    it('renders interval label in seconds', async () => {
      mockHealth.set({ schedule_mode: 'interval', schedule_interval_secs: 45, next_run_at: null });
      const screen = render(CycleStatusCard, { watchedCount: 3 });
      await expect.element(screen.container.querySelector('.type-body-sm')).toHaveTextContent(
        'every 45s',
      );
    });

    it('renders cron expression for cron mode', async () => {
      mockHealth.set({ schedule_mode: 'cron', schedule_cron: '0 */6 * * *', next_run_at: null });
      const screen = render(CycleStatusCard, { watchedCount: 3 });
      await expect.element(screen.container.querySelector('.type-body-sm')).toHaveTextContent(
        '0 */6 * * *',
      );
    });

    it('renders — for unknown schedule_mode', async () => {
      mockHealth.set({ schedule_mode: 'unknown_mode', next_run_at: null });
      const screen = render(CycleStatusCard, { watchedCount: 3 });
      await expect.element(screen.container.querySelector('.type-body-sm')).toHaveTextContent('—');
    });

    it('renders — for countdown when next_run_at is null', async () => {
      mockHealth.set({ schedule_mode: 'interval', schedule_interval_secs: 3600, next_run_at: null });
      const screen = render(CycleStatusCard, { watchedCount: 5 });
      await expect.element(screen.container.querySelector('.countdown')).toHaveTextContent('—');
    });

    it('renders countdown when next_run_at is set', async () => {
      vi.setSystemTime(new Date('2024-01-01T00:00:00Z'));
      const nextRun = new Date('2024-01-01T00:00:30Z').toISOString();
      mockHealth.set({ schedule_mode: 'interval', schedule_interval_secs: 60, next_run_at: nextRun });
      const screen = render(CycleStatusCard, { watchedCount: 0 });
      await expect.element(screen.container.querySelector('.countdown')).toHaveTextContent('in 30s');
    });

    it('includes watchedCount in the sub label', async () => {
      mockHealth.set({ schedule_mode: 'interval', schedule_interval_secs: 3600, next_run_at: null });
      const screen = render(CycleStatusCard, { watchedCount: 7 });
      await expect.element(screen.container.querySelector('.type-body-sm')).toHaveTextContent(
        '7 watched',
      );
    });
  });

  describe('running state', () => {
    const started_at = '2024-01-01T00:00:00Z';
    const progress = {
      started_at,
      phase: 'updating',
      scanned: 3,
      total: 10,
      current: 'my-container',
    };

    beforeEach(() => {
      vi.setSystemTime(new Date('2024-01-01T00:01:05Z'));
    });

    it('shows elapsed time', async () => {
      const screen = render(CycleStatusCard, { progress });
      await expect.element(screen.container.querySelector('.elapsed')).toHaveTextContent('1:05');
    });

    it('shows phase and completion percentage', async () => {
      const screen = render(CycleStatusCard, { progress });
      await expect.element(screen.container.querySelector('.muted')).toHaveTextContent(
        'updating · 30% complete',
      );
    });

    it('shows current container name', async () => {
      const screen = render(CycleStatusCard, { progress });
      await expect.element(screen.container.querySelector('.current')).toHaveTextContent(
        'my-container',
      );
    });

    it('shows scanned/total count', async () => {
      const screen = render(CycleStatusCard, { progress });
      await expect.element(screen.container.querySelector('.count')).toHaveTextContent('3/10');
    });

    it('updates elapsed after 1 second', async () => {
      const screen = render(CycleStatusCard, { progress });
      await expect.element(screen.container.querySelector('.elapsed')).toHaveTextContent('1:05');
      await vi.advanceTimersByTimeAsync(1000);
      await expect.element(screen.container.querySelector('.elapsed')).toHaveTextContent('1:06');
    });
  });
});
