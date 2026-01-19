// This file can be used to expose APIs to the renderer process
window.addEventListener('DOMContentLoaded', () => {
  // Example: Expose a version API
  window.versions = {
    node: () => process.versions.node,
    chrome: () => process.versions.chrome,
    electron: () => process.versions.electron,
  };
});
