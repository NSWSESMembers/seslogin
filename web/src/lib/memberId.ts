export function isValidMemberIdText(memberId: string): boolean {
  return (
    /^\d{8}$/.test(memberId)
  );
}
