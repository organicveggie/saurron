import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Sparkline from './Sparkline.svelte';

const W = 120;

function pt(updated, failed = 0, rolled_back = 0) {
  return { updated, failed, rolled_back };
}

describe('Sparkline', () => {
  it('renders no svg for empty data', () => {
    const { container } = render(Sparkline, { data: [] });
    expect(container.querySelector('svg')).toBeNull();
  });

  it('renders svg for non-empty data', () => {
    const { container } = render(Sparkline, { data: [pt(5)] });
    expect(container.querySelector('svg')).not.toBeNull();
  });

  it('single point centers x at W/2', () => {
    const { container } = render(Sparkline, { data: [pt(5)] });
    const pairs = container.querySelector('polyline').getAttribute('points').trim().split(' ');
    expect(pairs).toHaveLength(1);
    expect(pairs[0].startsWith(`${W / 2},`)).toBe(true);
  });

  it('polyline coordinate count matches data.length', () => {
    const data = [pt(5), pt(3), pt(7)];
    const { container } = render(Sparkline, { data });
    const pairs = container.querySelector('polyline').getAttribute('points').trim().split(' ');
    expect(pairs).toHaveLength(data.length);
  });

  it('omits area path for single-point data', () => {
    const { container } = render(Sparkline, { data: [pt(5)] });
    expect(container.querySelector('path.area')).toBeNull();
  });

  it('renders well-formed area path for multi-point data', () => {
    const { container } = render(Sparkline, { data: [pt(5), pt(3)] });
    const d = container.querySelector('path.area').getAttribute('d');
    expect(d.startsWith('M ')).toBe(true);
    expect(d).toContain(' L ');
    expect(d.endsWith(' Z')).toBe(true);
  });

  it('renders bad-dot circle when a point has failed > 0', () => {
    const { container } = render(Sparkline, { data: [pt(5, 1, 0)] });
    expect(container.querySelectorAll('circle.bad-dot').length).toBe(1);
  });

  it('renders bad-dot circle when a point has rolled_back > 0', () => {
    const { container } = render(Sparkline, { data: [pt(5, 0, 1)] });
    expect(container.querySelectorAll('circle.bad-dot').length).toBe(1);
  });

  it('renders no bad-dot circles for all-good data', () => {
    const { container } = render(Sparkline, { data: [pt(5), pt(3), pt(7)] });
    expect(container.querySelectorAll('circle.bad-dot').length).toBe(0);
  });

  it('renders bad-dot only for affected points in mixed data', () => {
    const { container } = render(Sparkline, { data: [pt(5), pt(3, 1, 0), pt(7)] });
    expect(container.querySelectorAll('circle.bad-dot').length).toBe(1);
  });
});
