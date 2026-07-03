export function isValidMemberIdText(memberId: string): boolean {
  return (
    memberId.startsWith("400") && memberId.length >= 8 && /^\d+$/.test(memberId)
  );
}
