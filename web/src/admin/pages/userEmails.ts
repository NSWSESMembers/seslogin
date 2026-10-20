// Parsing for the New user form's email box, which takes several addresses at
// once so a batch of users can be created with the same permissions.

// Deliberately loose: the browser's own `type="email"` check is the bar the
// single-email form used, and this only needs to catch typos like a missing @.
const EMAIL = /^[^\s@,;]+@[^\s@,;]+\.[^\s@,;]+$/;

export type ParsedEmails = {
  /** Valid addresses, trimmed, in the order entered, duplicates dropped. */
  emails: string[];
  /** Entries that don't look like an email address. */
  invalid: string[];
};

/**
 * Split free text into email addresses. Commas, semicolons, whitespace and
 * new lines all separate entries, so a column pasted from a spreadsheet or a
 * list copied out of an email client both work. Duplicates are compared
 * case-insensitively and the first spelling is kept.
 */
export function parseEmails(text: string): ParsedEmails {
  const emails: string[] = [];
  const invalid: string[] = [];
  const seen = new Set<string>();
  for (const raw of text.split(/[\s,;]+/)) {
    // "Name <a@b.com>" as copied from a mail client: keep the address.
    const entry = raw.replace(/^<|>$/g, "");
    if (!entry) continue;
    if (!EMAIL.test(entry)) {
      invalid.push(entry);
      continue;
    }
    const key = entry.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    emails.push(entry);
  }
  return { emails, invalid };
}
