export interface ProfileInfo {
  id: string;
  name: string;
  created_at: string;
  managed_by: string | null;
  password_hint: string | null;
  date_of_birth: string | null;
  color_index: number | null;
  country: string | null;
  address: string | null;
}

export interface ProfileCreateResult {
  profile: ProfileInfo;
  recovery_phrase: string[];
}

/** 8-color palette for profile visual identity (Spec 45). */
export const PROFILE_COLORS: string[] = [
  '#4A90D9', // Blue
  '#E07C4F', // Coral
  '#5BAE6E', // Green
  '#9B6DC6', // Purple
  '#D4A843', // Gold
  '#E06B8C', // Rose
  '#47A5A5', // Teal
  '#8B7355', // Warm brown
];

export type AppScreen =
  | 'loading'
  | 'trust'
  | 'profile_type_choice'
  | 'create'
  | 'picker'
  | 'unlock'
  | 'recovery_display'
  | 'welcome_tour'
  | 'recover'
  | 'app';
