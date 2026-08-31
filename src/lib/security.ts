export const LOCAL_PASSWORD_MIN_LENGTH = 6;
export const BACKUP_PASSWORD_MIN_LENGTH = 10;

export function passwordCharacterCount(value: string): number {
  return Array.from(value).length;
}
