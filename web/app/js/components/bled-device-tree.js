import { devices } from '../services/devices.js';
import { escapeHtml } from '../utils/formatters.js';

const sameSelection = (a, b) =>
  !!a && !!b &&
  a.deviceId === b.deviceId && a.kind === b.kind &&
  a.fixture === b.fixture && a.span === b.span &&
  a.channel === b.channel && a.segment === b.segment;

/**
 * Sidebar tree: DEVICE > (SD card, FIXTURE > SPAN). Fixtures and spans
 * come from each device's config file; none are shown if it could not be read.
 */
export class BledDeviceTree extends HTMLElement {
  connectedCallback() {
    this.collapsed = new Set();

    this._onChange = () => this._render();
    devices.addEventListener('devices-changed', this._onChange);
    devices.addEventListener('selection-changed', this._onChange);

    this.addEventListener('click', (e) => {
      const toggle = e.target.closest('[data-toggle]');
      if (toggle) {
        const key = toggle.dataset.toggle;
        if (!this.collapsed.delete(key)) this.collapsed.add(key);
        this._render();
        return;
      }

      const disconnect = e.target.closest('[data-disconnect]');
      if (disconnect) {
        devices.disconnectDevice(disconnect.dataset.disconnect);
        return;
      }

      const node = e.target.closest('[data-select]');
      if (node) devices.select(JSON.parse(node.dataset.select));
    });

    this._render();
  }

  disconnectedCallback() {
    devices.removeEventListener('devices-changed', this._onChange);
    devices.removeEventListener('selection-changed', this._onChange);
  }

  _node({ label, selection, depth, key = null, hasChildren = false, icon = '', actions = '' }) {
    const selected = sameSelection(devices.selection, selection);
    const expanded = hasChildren && !this.collapsed.has(key);
    const caret = hasChildren
      ? `<button class="tree-caret" data-toggle="${escapeHtml(key)}" aria-label="${expanded ? 'Collapse' : 'Expand'}">${expanded ? '▾' : '▸'}</button>`
      : '<span class="tree-caret"></span>';
    return `
      <li role="treeitem" ${hasChildren ? `aria-expanded="${expanded}"` : ''} aria-selected="${selected}">
        <div class="tree-row ${selected ? 'selected' : ''}" style="padding-left: ${0.25 + depth * 1.1}rem;">
          ${caret}
          <button class="tree-label" data-select="${escapeHtml(JSON.stringify(selection))}" title="${escapeHtml(label)}">
            <span class="tree-icon">${icon}</span>${escapeHtml(label)}
          </button>
          ${actions}
        </div>
      </li>`;
  }

  _renderDevice(dev) {
    const id = dev.id;
    const deviceKey = `d:${id}`;
    const fixtures = dev.config?.fixtures ?? [];
    const parts = [];

    parts.push(this._node({
      label: dev.displayName,
      selection: { deviceId: id, kind: 'device' },
      depth: 0, key: deviceKey, hasChildren: true, icon: '◉',
      actions: `<button class="tree-action" data-disconnect="${escapeHtml(id)}" title="Disconnect">✕</button>`,
    }));

    if (!this.collapsed.has(deviceKey)) {
      const children = [];
      children.push(this._node({
        label: 'SD card', selection: { deviceId: id, kind: 'sdcard' }, depth: 1, icon: '▤',
      }));

      for (const fix of fixtures) {
        const fixtureKey = `${deviceKey}:f:${fix.index}`;
        const hasSpans = fix.spans && fix.spans.length > 1;
        children.push(this._node({
          label: `${fix.name} (${fix.totalPixels} px)`,
          selection: { deviceId: id, kind: 'fixture', fixture: fix.index },
          depth: 1, key: fixtureKey, hasChildren: hasSpans, icon: '💡',
        }));
        if (hasSpans && !this.collapsed.has(fixtureKey)) {
          for (const sp of fix.spans) {
            children.push(this._node({
              label: `Ch ${sp.channel}: ${sp.start}..${sp.start + sp.length - 1} (${sp.length} px)`,
              selection: { deviceId: id, kind: 'span', fixture: fix.index, span: sp.index },
              depth: 2, icon: '▪',
            }));
          }
        }
      }

      // Legacy fallback if channels exist but no fixtures
      if (fixtures.length === 0 && dev.config?.channels?.length > 0) {
        for (const ch of dev.config.channels) {
          children.push(this._node({
            label: `${ch.name} (${ch.length} px)`,
            selection: { deviceId: id, kind: 'channel', channel: ch.index },
            depth: 1, icon: '≡',
          }));
        }
      }

      parts.push(`<ul role="group">${children.join('')}</ul>`);
    }
    return parts.join('');
  }

  _render() {
    const list = devices.list();
    const connecting = devices.pending > 0
      ? '<p class="tree-empty">Connecting…</p>' : '';
    this.innerHTML = `
      <nav class="tree" aria-label="Devices">
        <h2 class="card-title tree-heading">Devices</h2>
        ${list.length
          ? `<ul role="tree">${list.map((d) => `<li role="none">${this._renderDevice(d)}</li>`).join('')}</ul>`
          : '<p class="tree-empty">No devices connected.</p>'}
        ${connecting}
      </nav>`;
  }
}

customElements.define('bled-device-tree', BledDeviceTree);
