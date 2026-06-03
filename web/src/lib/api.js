const BASE = '/v1';

async function request(path) {
  const res = await fetch(BASE + path);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

export function getHistory(page = 1, perPage = 20) {
  return request(`/history?page=${page}&per_page=${perPage}`);
}

export function getHistoryById(id) {
  return request(`/history/${id}`);
}

export function getHealth() {
  return request('/health');
}

export function getContainers() {
  return request('/containers');
}
