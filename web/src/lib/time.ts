const fullDateTimeFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "short",
  timeStyle: "short",
  hourCycle: "h23",
});

const shortTimeFormatter = new Intl.DateTimeFormat(undefined, {
  timeStyle: "short",
  hourCycle: "h23",
});

const dayOfWeekFormatter = new Intl.DateTimeFormat(undefined, {
  weekday: "short",
});

export function formatFullDateTime(date: Date): string {
  return fullDateTimeFormatter.format(date);
}

const longDateTimeWithWeekdayFormatter = new Intl.DateTimeFormat(undefined, {
  weekday: "long",
  year: "numeric",
  month: "long",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});

export function formatLongDateTimeWithWeekday(date: Date): string {
  return longDateTimeWithWeekdayFormatter.format(date);
}

export function formatTimeDiff(start: Date, end: Date): string {
  const diffMs = end.getTime() - start.getTime();
  const diffSecs = Math.round(diffMs / 1000);
  return formatSeconds(diffSecs);
}

export function formatSeconds(seconds: number): string {
  const diffMins = Math.round(seconds / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffDays >= 2) {
    return `${diffDays} days`;
  }
  if (diffHours >= 2) {
    return `${diffHours} hours`;
  }
  return `${diffMins} mins`;
}

/**
 * Compact duration for diagnostics ("42s", "3m 05s", "2h 14m", "3d 4h"). Unlike
 * `formatSeconds` this keeps second-level detail, so a just-happened event doesn't
 * read as "0 mins". Negative inputs are clamped to zero.
 */
export function formatShortDuration(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const secs = total % 60;
  const mins = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600) % 24;
  const days = Math.floor(total / 86400);

  if (days > 0) {
    return `${days}d ${hours}h`;
  }
  if (hours > 0) {
    return `${hours}h ${mins.toString().padStart(2, "0")}m`;
  }
  if (mins > 0) {
    return `${mins}m ${secs.toString().padStart(2, "0")}s`;
  }
  return `${secs}s`;
}

export function formatTime(date: Date): string {
  return shortTimeFormatter.format(date);
}

export function formatDayDate(date: Date): string {
  return `${dayOfWeekFormatter.format(date)} ${date.getDate()}/${date.getMonth() + 1}`;
}

export function formatDayDateTime(date: Date): string {
  return `${formatDayDate(date)} ${formatTime(date)}`;
}

export function formatTimeOfDay(hours: number, minutes: number): string {
  const d = new Date();
  d.setHours(hours, minutes, 0, 0);
  return shortTimeFormatter.format(d);
}

export function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

export function dateToInputDateTimeLocal(date: Date): string {
  const year = date.getFullYear();
  const month = (date.getMonth() + 1).toString().padStart(2, "0");
  const day = date.getDate().toString().padStart(2, "0");
  const hours = date.getHours().toString().padStart(2, "0");
  const minutes = date.getMinutes().toString().padStart(2, "0");

  return `${year}-${month}-${day}T${hours}:${minutes}`;
}
