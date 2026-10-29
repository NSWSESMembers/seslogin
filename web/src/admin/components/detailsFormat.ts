// Shown in place of a value the SES sync has not supplied.
export const NONE = "—";

export function mapUrl(latitude: number, longitude: number): string {
  return `https://www.google.com/maps?q=${latitude},${longitude}`;
}
