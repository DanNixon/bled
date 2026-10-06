export const formatBytes = (bytes) => {
  const n = Number(bytes);
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
};

export const formatUptime = (ms) => {
  if (ms == null) return '—';
  const sec = Math.floor(ms / 1000);
  const d = Math.floor(sec / 86400);
  const h = Math.floor((sec % 86400) / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  let formatted = '';
  if (d > 0) formatted += `${d}d `;
  if (h > 0 || d > 0) formatted += `${h}h `;
  if (m > 0 || h > 0 || d > 0) formatted += `${m}m `;
  formatted += `${s}s (${ms.toLocaleString()} ms)`;
  return formatted;
};

export const renderHexDump = (uint8arr, maxBytes = 2048) => {
  if (!uint8arr) return '';
  const len = Math.min(uint8arr.length, maxBytes);
  let out = '';
  for (let i = 0; i < len; i += 16) {
    const offset = i.toString(16).padStart(6, '0');
    const slice = uint8arr.slice(i, Math.min(i + 16, len));
    const hex = Array.from(slice).map(b => b.toString(16).padStart(2, '0')).join(' ').padEnd(48, ' ');
    const ascii = Array.from(slice).map(b => (b >= 32 && b <= 126) ? String.fromCharCode(b) : '.').join('');
    out += `${offset}:  ${hex}  |${ascii}|\n`;
  }
  if (uint8arr.length > maxBytes) {
    out += `... (${uint8arr.length - maxBytes} more bytes truncated in preview)`;
  }
  return out;
};

export function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]
  ));
}
