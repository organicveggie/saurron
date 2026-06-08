import { http, HttpResponse } from 'msw';
import { cycles, historyPage } from '../fixtures/cycles.js';
import { containers } from '../fixtures/containers.js';
import { health } from '../fixtures/health.js';

export const handlers = [
  http.get('/v1/health', () => HttpResponse.json(health)),
  http.get('/v1/containers', () => HttpResponse.json(containers)),
  http.get('/v1/history/:id', ({ params }) => {
    const cycle = cycles.find((c) => String(c.id) === params.id);
    return cycle ? HttpResponse.json(cycle) : new HttpResponse(null, { status: 404 });
  }),
  http.get('/v1/history', ({ request }) => {
    const url = new URL(request.url);
    const page = Number(url.searchParams.get('page') ?? '1');
    const perPage = Number(url.searchParams.get('per_page') ?? '20');
    return HttpResponse.json(historyPage(page, perPage));
  }),
];
