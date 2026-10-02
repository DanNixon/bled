class LoggerService extends EventTarget {
  constructor() {
    super();
    this.history = [];
  }

  log(msg, level = 'info') {
    const ts = new Date().toISOString().substring(11, 19);
    const prefix = level === 'error' ? '❌ ' : level === 'success' ? '✓ ' : '';
    const formatted = `[${ts}] ${prefix}${msg}`;
    console.log(formatted);
    this.history.push({ msg, level, ts, formatted });
    this.dispatchEvent(new CustomEvent('log', { detail: { msg, level, ts, formatted } }));
  }

  clear() {
    this.history = [];
    this.dispatchEvent(new CustomEvent('clear'));
  }
}

export const logger = new LoggerService();
export const log = (msg, level = 'info') => logger.log(msg, level);
export const clearLog = () => logger.clear();
