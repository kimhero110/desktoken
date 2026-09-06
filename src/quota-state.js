// Shared by the desktop renderer and dependency-free regression tests.
function quotaIsStale(snapshot, nowSeconds = Date.now() / 1000) {
  return nowSeconds - snapshot.fetched_at > (snapshot.stale_after_secs ?? 600);
}
function quotaSummary(snapshots, nowSeconds = Date.now() / 1000) {
  let worst = null;
  let unavailable = 0;
  for (const s of snapshots) {
    if (s.error || quotaIsStale(s, nowSeconds) || !s.windows.length) {
      unavailable++;
      continue;
    }
    const valid = s.windows.filter(w => Number.isFinite(w.used_percent));
    if (!valid.length) { unavailable++; continue; }
    for (const w of valid) worst = Math.max(worst ?? 0, w.used_percent);
  }
  return { worst, unavailable };
}
if (typeof module !== "undefined") module.exports = { quotaSummary, quotaIsStale };
