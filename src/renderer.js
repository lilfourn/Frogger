import './index.css';

/**
 * Renderer scaffold for the file manager UI.
 *
 * Responsibility: build the static DOM shell and expose a stable set of
 * element references and helpers that the Wave 2 wiring task can use to
 * populate sidebar items, breadcrumbs, table rows, and status text.
 *
 * No IPC, no real data, no navigation logic here.
 */

const PLACEHOLDER_LOCATIONS = [
  { label: 'Home', path: '~' },
  { label: 'Documents', path: '~/Documents' },
  { label: 'Downloads', path: '~/Downloads' },
  { label: 'Desktop', path: '~/Desktop' },
  { label: 'Applications', path: '/Applications' },
];

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
    ...PLACEHOLDER_LOCATIONS.map(buildSidebarItem),
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
      el('th', { class: 'col-name', scope: 'col' }, 'Name'),
      el('th', { class: 'col-kind', scope: 'col' }, 'Kind'),
      el('th', { class: 'col-size', scope: 'col' }, 'Size'),
      el('th', { class: 'col-date', scope: 'col' }, 'Date Modified'),
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

function render(root) {
  root.replaceChildren(
    buildSidebar(),
    buildToolbar(),
    buildFileList(),
    buildStatusBar(),
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
};

export { el, buildSidebarItem };
