export const CHECK_INTERVAL = 60 * 60 * 1000;

// A single startup check, not a timer. Invalid/future clocks must not disable updates.
export function shouldCheckAtStartup(last: string | null, now = Date.now()): boolean {
  if (last === null) return true;
  const checked = Number(last);
  return !Number.isFinite(checked) || checked <= 0 || checked > now || now - checked >= CHECK_INTERVAL;
}

export function downloadPercent(downloaded: number, total?: number): number | undefined {
  if (!total || total <= 0) return undefined;
  return Math.min(100, Math.max(0, Math.round(downloaded / total * 100)));
}
