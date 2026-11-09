// Native Messaging keeps the browser extension isolated from the keyboard hook.
const HOST = 'com.fasthomez.uvie_smart_switch';
const BROWSER = navigator.userAgent.includes('Edg/') ? 'msedge.exe' :
  navigator.userAgent.includes('Brave/') ? 'brave.exe' : 'chrome.exe';
let port;
async function session() {
  const obj = await chrome.storage.session.get('uvieSession');
  if (obj.uvieSession) return obj.uvieSession;
  const id = crypto.randomUUID();
  await chrome.storage.session.set({uvieSession: id});
  return id;
}
function nativePort() {
  if (!port) {
    port = chrome.runtime.connectNative(HOST);
    port.onDisconnect.addListener(() => { port = null; });
  }
  return port;
}
async function reportActive() {
  const windows = await chrome.windows.getLastFocused({populate: true});
  if (!windows.focused || windows.type !== 'normal') return;
  const tab = windows.tabs?.find(t => t.active);
  if (!tab?.url || !/^https?:\/\//i.test(tab.url)) return;
  nativePort().postMessage({browser: BROWSER, session: await session(),
    window_id: windows.id, tab_id: tab.id, url: tab.url});
}
chrome.tabs.onActivated.addListener(reportActive);
chrome.tabs.onUpdated.addListener((_id, info) => { if (info.url || info.status === 'complete') reportActive(); });
chrome.windows.onFocusChanged.addListener(reportActive);
chrome.runtime.onStartup.addListener(reportActive);
chrome.runtime.onInstalled.addListener(reportActive);
chrome.alarms.create('uvie-heartbeat', {periodInMinutes: 0.5});
chrome.alarms.onAlarm.addListener(a => {if(a.name === 'uvie-heartbeat') reportActive();});
reportActive();
