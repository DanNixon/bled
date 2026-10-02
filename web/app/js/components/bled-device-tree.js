import { devices } from '../services/devices.js';
import { escapeHtml } from '../utils/formatters.js';

const sameSelection = (a, b) =>
  !!a && !!b &&
  a.deviceId === b.deviceId && a.kind === b.kind &&
  a.channel === b.channel && a.segment === b.segment;

/**
 * Sidebar tree: DEVICE > (SD card, CHANNEL > SEGMENT). Channels and segments
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
    const channels = dev.config?.channels ?? [];
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
      for (const ch of channels) {
        const channelKey = `${deviceKey}:c:${ch.index}`;
        children.push(this._node({
          label: ch.name,
          selection: { deviceId: id, kind: 'channel', channel: ch.index },
          depth: 1, key: channelKey, hasChildren: ch.segments.length > 0, icon: '≡',
        }));
        if (!this.collapsed.has(channelKey)) {
          for (const seg of ch.segments) {
            children.push(this._node({
              label: seg.name,
              selection: { deviceId: id, kind: 'segment', channel: ch.index, segment: seg.index },
              depth: 2, icon: '▪',
            }));
          }
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
