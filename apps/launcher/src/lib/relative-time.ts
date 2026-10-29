// "2 hours ago" / "Yesterday" / "3 days ago" / "Last week" / date — for play history lines.

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** For mid-sentence use ("Last played yesterday"): lower-cases a relative phrase,
 *  keeps a calendar date as is. */
export function relativeTimeInSentence(unixSeconds: number, now?: number): string {
  const text = relativeTime(unixSeconds, now);
  return /\d{4}/.test(text) ? text : text.charAt(0).toLowerCase() + text.slice(1);
}

/** Relative description of a unix-seconds timestamp; `now` is injectable for tests. */
export function relativeTime(unixSeconds: number, now: number = Date.now() / 1000): string {
  const elapsed = Math.max(0, now - unixSeconds);
  if (elapsed < MINUTE) return "Just now";
  if (elapsed < HOUR) {
    const minutes = Math.floor(elapsed / MINUTE);
    return minutes === 1 ? "1 minute ago" : `${minutes} minutes ago`;
  }
  if (elapsed < DAY) {
    const hours = Math.floor(elapsed / HOUR);
    return hours === 1 ? "1 hour ago" : `${hours} hours ago`;
  }
  const days = Math.floor(elapsed / DAY);
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  if (days < 14) return "Last week";
  return new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}
