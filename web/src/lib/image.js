export function imageTagOnly(ref) {
  if (!ref) return '—';
  const noDigest = ref.split('@')[0];
  const lastSlash = noDigest.lastIndexOf('/');
  return noDigest.slice(lastSlash + 1);
}

export function imageShortDigest(ref) {
  if (!ref) return '';
  const at = ref.indexOf('@sha256:');
  if (at < 0) return '';
  return ref.slice(at + 8, at + 15);
}
