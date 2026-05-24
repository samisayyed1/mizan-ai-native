import { formatDistanceToNowStrict, fromUnixTime, isValid } from "date-fns";

/** Relative "x ago" label from a unix-seconds publish time. Empty when invalid. */
export function relativeNewsTime(publishedUnixSeconds: number): string {
  if (!publishedUnixSeconds || publishedUnixSeconds <= 0) return "";
  const date = fromUnixTime(publishedUnixSeconds);
  if (!isValid(date)) return "";
  return formatDistanceToNowStrict(date, { addSuffix: true });
}
