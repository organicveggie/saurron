import { writable } from 'svelte/store';

const initial = document.documentElement.dataset.theme ?? 'light';
export const theme = writable(initial);

theme.subscribe((t) => {
  document.documentElement.dataset.theme = t;
  localStorage.setItem('theme', t);
});
