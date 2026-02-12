export interface ProfileInfo {
  id: string;
  name: string;
  created_at: string;
  managed_by: string | null;
  password_hint: string | null;
}

export interface ProfileCreateResult {
  profile: ProfileInfo;
  recovery_phrase: string[];
}

export type AppScreen =
  | 'loading'
  | 'trust'
  | 'create'
  | 'picker'
  | 'unlock'
  | 'recovery_display'
  | 'recover'
  | 'app';
