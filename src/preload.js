// See the Electron documentation for details on how to use preload scripts:
// https://www.electronjs.org/docs/latest/tutorial/process-model#preload-scripts
import { contextBridge, ipcRenderer } from 'electron';

contextBridge.exposeInMainWorld('fileSystem', {
  getHome: () => ipcRenderer.invoke('fs:getHome'),
  getCommonLocations: () => ipcRenderer.invoke('fs:getCommonLocations'),
  listVolumes: () => ipcRenderer.invoke('fs:listVolumes'),
  listDirectory: (absPath, opts) => ipcRenderer.invoke('fs:listDirectory', absPath, opts),
  openPath: (absPath) => ipcRenderer.invoke('fs:openPath', absPath),
  revealInOS: (absPath) => ipcRenderer.invoke('fs:revealInOS', absPath),
});
