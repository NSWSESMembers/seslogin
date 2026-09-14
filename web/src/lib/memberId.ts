/** Member IDs are a fixed-length run of digits. */
export const MEMBER_ID_LENGTH = 8;

const MEMBER_ID_PATTERN = new RegExp(`^\\d{${MEMBER_ID_LENGTH}}$`);

export function isValidMemberIdText(memberId: string): boolean {
  return MEMBER_ID_PATTERN.test(memberId);
}
