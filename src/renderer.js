import './index.css';

/**
 * Renderer for the file manager UI.
 *
 * Builds the static DOM shell, then wires it to `window.fileSystem` for
 * navigation, listing, sorting, opening, and reveal-in-OS.
 */

function el(tag, props = {}, children = []) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined || value === null) continue;
    if (key === 'class') node.className = value;
    else if (key === 'dataset') {
      for (const [k, v] of Object.entries(value)) node.dataset[k] = v;
    } else if (key in node) node[key] = value;
    else node.setAttribute(key, value);
  }
  for (const child of [].concat(children)) {
    if (child == null) continue;
    node.appendChild(typeof child === 'string' ? document.createTextNode(child) : child);
  }
  return node;
}

function buildSidebarItem({ label, path }) {
  return el(
    'button',
    { class: 'sidebar-item', type: 'button', dataset: { path } },
    label,
  );
}

function buildSidebar() {
  const locations = el('div', { id: 'sidebar-locations', class: 'sidebar-section' }, [
    el('div', { class: 'sidebar-section-title' }, 'Locations'),
  ]);
  const volumes = el('div', { id: 'sidebar-volumes', class: 'sidebar-section' }, [
    el('div', { class: 'sidebar-section-title' }, 'Volumes'),
  ]);
  return el('aside', { id: 'sidebar' }, [locations, volumes]);
}

function buildToolbar() {
  const back = el('button', {
    id: 'nav-back',
    type: 'button',
    class: 'toolbar-button',
    title: 'Back',
    disabled: true,
  }, '‹');
  const forward = el('button', {
    id: 'nav-forward',
    type: 'button',
    class: 'toolbar-button',
    title: 'Forward',
    disabled: true,
  }, '›');
  const breadcrumbs = el('div', { id: 'breadcrumbs' });
  const spacer = el('div', { id: 'toolbar-spacer' });
  return el('header', { id: 'toolbar' }, [back, forward, breadcrumbs, spacer]);
}

function buildFileList() {
  const thead = el('thead', {}, [
    el('tr', {}, [
      el('th', { class: 'col-name', scope: 'col', dataset: { sort: 'name' } }, 'Name'),
      el('th', { class: 'col-kind', scope: 'col', dataset: { sort: 'kind' } }, 'Kind'),
      el('th', { class: 'col-size', scope: 'col', dataset: { sort: 'size' } }, 'Size'),
      el('th', { class: 'col-date', scope: 'col', dataset: { sort: 'modified' } }, 'Date Modified'),
    ]),
  ]);
  const tbody = el('tbody', { id: 'file-list-body' });
  const table = el('table', { id: 'file-list' }, [thead, tbody]);

  const empty = el('div', { id: 'empty-state', hidden: true }, [
    el('span', { id: 'empty-state-text' }, 'No items'),
  ]);
  const error = el('div', { id: 'error-state', hidden: true }, [
    el('span', { id: 'error-state-text' }, ''),
  ]);

  return el('main', { id: 'content' }, [table, empty, error]);
}

function buildStatusBar() {
  return el('footer', { id: 'status-bar' }, [
    el('span', { id: 'status-text' }, 'Ready'),
  ]);
}

function buildContextMenu() {
  return el('div', { id: 'context-menu', class: 'context-menu', hidden: true }, [
    el('button', { type: 'button', class: 'context-menu-item', dataset: { action: 'open' } }, 'Open'),
    el('button', { type: 'button', class: 'context-menu-item', dataset: { action: 'reveal' } }, 'Reveal in Finder'),
  ]);
}

function render(root) {
  root.replaceChildren(
    buildSidebar(),
    buildToolbar(),
    buildFileList(),
    buildStatusBar(),
    buildContextMenu(),
  );
}

const app = document.getElementById('app');
if (app) render(app);

/**
 * Minimal hooks the wiring task can import. Kept intentionally thin:
 * Wave 2 will read/write these elements directly.
 */
export const ui = {
  get app() { return document.getElementById('app'); },
  get sidebar() { return document.getElementById('sidebar'); },
  get sidebarLocations() { return document.getElementById('sidebar-locations'); },
  get sidebarVolumes() { return document.getElementById('sidebar-volumes'); },
  get toolbar() { return document.getElementById('toolbar'); },
  get navBack() { return document.getElementById('nav-back'); },
  get navForward() { return document.getElementById('nav-forward'); },
  get breadcrumbs() { return document.getElementById('breadcrumbs'); },
  get toolbarSpacer() { return document.getElementById('toolbar-spacer'); },
  get fileList() { return document.getElementById('file-list'); },
  get fileListBody() { return document.getElementById('file-list-body'); },
  get statusBar() { return document.getElementById('status-bar'); },
  get statusText() { return document.getElementById('status-text'); },
  get emptyState() { return document.getElementById('empty-state'); },
  get emptyStateText() { return document.getElementById('empty-state-text'); },
  get errorState() { return document.getElementById('error-state'); },
  get errorStateText() { return document.getElementById('error-state-text'); },
  get contextMenu() { return document.getElementById('context-menu'); },
};

const state = {
  history: [],
  historyIndex: -1,
  currentPath: null,
  entries: [],
  showHidden: false,
  sort: { column: 'name', direction: 'asc' },
  selectedPath: null,
  contextTarget: null,
};

function formatSize(bytes) {
  if (!Number.isFinite(bytes) || bytes < 0) return '';
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i += 1; }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

function formatDate(ms) {
  if (!Number.isFinite(ms)) return '';
  try { return new Date(ms).toLocaleString(); } catch { return ''; }
}

function compareEntries(a, b) {
  if (a.isDirectory !== b.isDirectory) return a.isDirectory ? -1 : 1;
  const { column, direction } = state.sort;
  const dir = direction === 'asc' ? 1 : -1;
  let cmp = 0;
  if (column === 'name') cmp = a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
  else if (column === 'kind') cmp = (a.kind || '').localeCompare(b.kind || '', undefined, { sensitivity: 'base' });
  else if (column === 'size') cmp = (a.size || 0) - (b.size || 0);
  else if (column === 'modified') cmp = (a.modifiedMs || 0) - (b.modifiedMs || 0);
  if (cmp === 0) cmp = a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
  return cmp * dir;
}

function visibleEntries() {
  const filtered = state.showHidden
    ? state.entries
    : state.entries.filter((e) => !e.isHidden);
  return [...filtered].sort(compareEntries);
}

function renderSortIndicators() {
  document.querySelectorAll('#file-list thead th').forEach((th) => {
    th.classList.remove('is-sorted-asc', 'is-sorted-desc');
    if (th.dataset.sort === state.sort.column) {
      th.classList.add(state.sort.direction === 'asc' ? 'is-sorted-asc' : 'is-sorted-desc');
    }
  });
}

function renderBreadcrumbs(absPath) {
  const bc = ui.breadcrumbs;
  bc.replaceChildren();
  const parts = [];
  if (!absPath) {
    return;
  } else if (absPath.startsWith('/')) {
    parts.push({ label: '/', path: '/' });
    let acc = '';
    for (const seg of absPath.split('/').filter(Boolean)) {
      acc += '/' + seg;
      parts.push({ label: seg, path: acc });
    }
  } else {
    const segs = absPath.split(/[\\/]/).filter(Boolean);
    let acc = '';
    segs.forEach((seg, i) => {
      acc = i === 0 ? seg + '\\' : acc + seg + '\\';
      parts.push({ label: seg, path: acc });
    });
  }
  parts.forEach((p, i) => {
    if (i > 0) bc.appendChild(el('span', { class: 'breadcrumb-separator' }, '▸'));
    bc.appendChild(el('button', {
      type: 'button',
      class: 'breadcrumb-item',
      dataset: { path: p.path },
    }, p.label));
  });
}

function renderFileList() {
  const tbody = ui.fileListBody;
  tbody.replaceChildren();
  const visible = visibleEntries();
  for (const entry of visible) {
    const tr = el('tr', { dataset: { path: entry.path, isDir: entry.isDirectory ? '1' : '0' } }, [
      el('td', { class: 'col-name' }, entry.name),
      el('td', { class: 'col-kind' }, entry.kind || (entry.isDirectory ? 'Folder' : 'File')),
      el('td', { class: 'col-size' }, entry.isDirectory ? '—' : formatSize(entry.size)),
      el('td', { class: 'col-date' }, formatDate(entry.modifiedMs)),
    ]);
    if (entry.path === state.selectedPath) tr.classList.add('is-selected');
    tbody.appendChild(tr);
  }
  ui.errorState.hidden = true;
  ui.emptyState.hidden = visible.length > 0;

  const total = state.entries.length;
  const word = visible.length === 1 ? 'item' : 'items';
  let text = `${visible.length} ${word}`;
  if (!state.showHidden && total !== visible.length) text += `, ${total} total`;
  ui.statusText.textContent = text;
}

function renderError(message) {
  ui.fileListBody.replaceChildren();
  ui.emptyState.hidden = true;
  ui.errorState.hidden = false;
  ui.errorStateText.textContent = message || 'Unable to read directory';
  ui.statusText.textContent = '';
}

function renderNavButtons() {
  ui.navBack.disabled = state.historyIndex <= 0;
  ui.navForward.disabled = state.historyIndex >= state.history.length - 1;
}

function renderSidebarSelection() {
  document.querySelectorAll('.sidebar-item').forEach((b) => {
    b.classList.toggle('is-selected', b.dataset.path === state.currentPath);
  });
}
