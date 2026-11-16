// 화면에 열어 주는 기능 목록. 화면(렌더러)은 Node·파일·프로세스에 직접 닿지 못하고 여기 있는 것만 쓴다.

'use strict';

const { contextBridge, ipcRenderer } = require('electron');

const CHANNELS = ['model', 'state'];

contextBridge.exposeInMainWorld('tacet', {
  getState: () => ipcRenderer.invoke('state:get'),
  saveSetup: (setup) => ipcRenderer.invoke('setup:save', setup),
  resetSetup: () => ipcRenderer.invoke('setup:reset'),
  resetAll: () => ipcRenderer.invoke('settings:reset'),
  command: (type) => ipcRenderer.invoke('backend:command', type),
  setDeveloper: (on) => ipcRenderer.invoke('dev:set', on),
  chooseConfig: () => ipcRenderer.invoke('dev:choose-config'),
  clearConfig: () => ipcRenderer.invoke('dev:clear-config'),
  openDataFolder: () => ipcRenderer.invoke('dev:open-data-folder'),
  openWhitelist: () => ipcRenderer.invoke('dev:open-whitelist'),
  restart: () => ipcRenderer.invoke('dev:restart'),
  startCalibration: () => ipcRenderer.invoke('calibration:start'),
  calibrationCommand: (type) => ipcRenderer.invoke('calibration:command', type),
  calibrationSelect: (bluetooth, wifi) => ipcRenderer.invoke('calibration:select', bluetooth, wifi),
  finishCalibration: (save) => ipcRenderer.invoke('calibration:finish', save),
  on: (channel, callback) => {
    if (!CHANNELS.includes(channel)) return () => {};
    const listener = (_event, payload) => callback(payload);
    ipcRenderer.on(channel, listener);
    return () => ipcRenderer.removeListener(channel, listener);
  },
});
