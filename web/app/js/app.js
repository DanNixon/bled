import init, * as bled from '../pkg/bled_wasm.js';
import { log } from './services/logger.js';

// Register all Web Components
import './components/bled-header.js';
import './components/bled-device-tree.js';
import './components/bled-main-panel.js';
import './components/bled-activity-log.js';

// Initialize WebAssembly runtime
try {
  await init();
  log("WebAssembly protocol runtime loaded successfully.", 'success');
  log(`Service UUID: ${bled.service_uuid()}`);
  document.querySelector('bled-header')?.setGitRevision(bled.git_revision());
} catch (e) {
  log(`Failed to initialize WebAssembly: ${e}`, 'error');
}
