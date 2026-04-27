import { app, BrowserWindow, ipcMain, shell } from 'electron';
import path from 'node:path';
import fs from 'node:fs/promises';
import os from 'node:os';
import started from 'electron-squirrel-startup';

// Handle creating/removing shortcuts on Windows when installing/uninstalling.
if (started) {
  app.quit();
}

const createWindow = () => {
  // Create the browser window.
  const mainWindow = new BrowserWindow({
    width: 800,
    height: 600,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
    },
  });

  // and load the index.html of the app.
  if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
    mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
  } else {
    mainWindow.loadFile(path.join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}/index.html`));
  }

  // Open the DevTools.
  mainWindow.webContents.openDevTools();
};

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.whenReady().then(() => {
  registerFileSystemHandlers();
  createWindow();

  // On OS X it's common to re-create a window in the app when the
  // dock icon is clicked and there are no other windows open.
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

// Quit when all windows are closed, except on macOS. There, it's common
// for applications and their menu bar to stay active until the user quits
// explicitly with Cmd + Q.
app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

// File system service: IPC handlers exposed to the renderer via the preload bridge.
// All FS access happens here in main; the renderer only calls IPC.

function ensureAbsolute(p) {
  if (typeof p !== 'string' || !path.isAbsolute(p)) {
    throw new Error('Path must be an absolute string');
  }
  return path.normalize(p);
}

function kindFor(name, isDirectory) {
  if (isDirectory) return 'Folder';
  const ext = path.extname(name);
  if (!ext) return 'File';
  return ext.slice(1).toUpperCase();
}

async function buildEntry(dirPath, dirent) {
  const entryPath = path.join(dirPath, dirent.name);
  const isSymlink = dirent.isSymbolicLink();
  let stats;
  try {
    stats = await fs.stat(entryPath);
  } catch {
    // Broken symlink or permission error — fall back to lstat so the entry still appears.
    try {
      stats = await fs.lstat(entryPath);
    } catch {
      return null;
    }
  }
  const isDirectory = stats.isDirectory();
  return {
    name: dirent.name,
    path: entryPath,
    isDirectory,
    isSymlink,
    isHidden: dirent.name.startsWith('.'),
    size: isDirectory ? 0 : stats.size,
    modifiedMs: stats.mtimeMs,
    kind: kindFor(dirent.name, isDirectory),
  };
}

function safeAppPath(name) {
  try {
    return app.getPath(name);
  } catch {
    return null;
  }
}

async function getCommonLocations() {
  const home = app.getPath('home');
  const candidates = [
    { id: 'home', label: 'Home', path: home },
    { id: 'desktop', label: 'Desktop', path: safeAppPath('desktop') },
    { id: 'documents', label: 'Documents', path: safeAppPath('documents') },
    { id: 'downloads', label: 'Downloads', path: safeAppPath('downloads') },
  ];
  if (process.platform === 'darwin') {
    candidates.push({ id: 'applications', label: 'Applications', path: '/Applications' });
  } else if (process.platform === 'linux') {
    candidates.push({ id: 'applications', label: 'Applications', path: '/usr/share/applications' });
  }
  const results = [];
  for (const c of candidates) {
    if (!c.path) continue;
    try {
      const s = await fs.stat(c.path);
      if (s.isDirectory()) results.push(c);
    } catch {
      // skip locations that don't exist or aren't accessible
    }
  }
  return results;
}

async function listVolumes() {
  const volumes = [];
  if (process.platform === 'darwin') {
    // macOS: enumerate /Volumes. Includes the root volume alias (e.g. "Macintosh HD")
    // for parity with Finder's sidebar; the renderer can choose to filter it.
    try {
      const entries = await fs.readdir('/Volumes', { withFileTypes: true });
      for (const e of entries) {
        if (e.isDirectory() || e.isSymbolicLink()) {
          volumes.push({ name: e.name, path: path.join('/Volumes', e.name) });
        }
      }
    } catch {
      // /Volumes unreadable — return what we have
    }
  } else if (process.platform === 'linux') {
    const roots = [];
    const userMedia = `/media/${os.userInfo().username}`;
    roots.push(userMedia, '/media', '/mnt');
    const seen = new Set();
    for (const root of roots) {
      try {
        const entries = await fs.readdir(root, { withFileTypes: true });
        for (const e of entries) {
          if (!e.isDirectory()) continue;
          const full = path.join(root, e.name);
          if (seen.has(full)) continue;
          seen.add(full);
          volumes.push({ name: e.name, path: full });
        }
      } catch {
        // root doesn't exist or isn't readable
      }
    }
  } else if (process.platform === 'win32') {
    for (let code = 'A'.charCodeAt(0); code <= 'Z'.charCodeAt(0); code++) {
      const letter = String.fromCharCode(code);
      const drivePath = `${letter}:\\`;
      try {
        await fs.access(drivePath);
        volumes.push({ name: `${letter}:`, path: drivePath });
      } catch {
        // drive not present
      }
    }
  }
  return volumes;
}

async function listDirectory(absPath, opts) {
  const dir = ensureAbsolute(absPath);
  const showHidden = Boolean(opts && opts.showHidden);
  const dirents = await fs.readdir(dir, { withFileTypes: true });
  const results = await Promise.all(dirents.map((d) => buildEntry(dir, d)));
  const entries = results
    .filter((e) => e !== null)
    .filter((e) => (showHidden ? true : !e.isHidden));
  return { path: dir, entries };
}

function registerFileSystemHandlers() {
  ipcMain.handle('fs:getHome', () => app.getPath('home'));
  ipcMain.handle('fs:getCommonLocations', () => getCommonLocations());
  ipcMain.handle('fs:listVolumes', () => listVolumes());
  ipcMain.handle('fs:listDirectory', (_event, absPath, opts) => listDirectory(absPath, opts));
  ipcMain.handle('fs:openPath', async (_event, absPath) => {
    try {
      const target = ensureAbsolute(absPath);
      const error = await shell.openPath(target);
      return error ? { ok: false, error } : { ok: true };
    } catch (err) {
      return { ok: false, error: err && err.message ? err.message : String(err) };
    }
  });
  ipcMain.handle('fs:revealInOS', (_event, absPath) => {
    shell.showItemInFolder(ensureAbsolute(absPath));
  });
}
