export const CHECK_INTERVAL = 60 * 60 * 1000;

// Invalid/future clocks must not disable checks after startup or resume.
export function shouldCheckAtStartup(last: string | null, now = Date.now()): boolean {
  if (last === null) return true;
  const checked = Number(last);
  return !Number.isFinite(checked) || checked <= 0 || checked > now || now - checked >= CHECK_INTERVAL;
}

export function downloadPercent(downloaded: number, total?: number): number | undefined {
  if (!total || total <= 0) return undefined;
  return Math.min(100, Math.max(0, Math.round(downloaded / total * 100)));
}

// Persist successful checks; keep failed-attempt throttling only in this process,
// so a relaunch can retry a failed network request without a rapid retry loop.
export function nextBackgroundCheckDelay(lastSuccess: string | null, lastAttempt: number, now = Date.now()): number {
  const successDelay = shouldCheckAtStartup(lastSuccess, now) ? 0 : Math.max(0, Number(lastSuccess) + CHECK_INTERVAL - now);
  const attemptDelay = Number.isFinite(lastAttempt) && lastAttempt > 0 && lastAttempt <= now ? Math.max(0, lastAttempt + CHECK_INTERVAL - now) : 0;
  return Math.max(successDelay, attemptDelay);
}
