let wakeLock = null;

async function requestWakeLock() {
  try {
    wakeLock = await navigator.wakeLock.request("screen");
  } catch (err) {
    console.log(`Failed to disable screen lock:\n\t${err.name}: ${err.message}`);
  }
}

async function handleVisibilityChange() {
  if (wakeLock !== null && document.visibilityState === 'visible') {
    await requestWakeLock();
  }
}

document.addEventListener("DOMContentLoaded", requestWakeLock);
document.addEventListener("visibilityChange", handleVisibilityChange);

// vim: set ts=4 sts=4 sw=4 et
