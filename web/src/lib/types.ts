export interface MediaState {
  status: string;
  title?: string;
  artist?: string;
  album?: string;
  art_url?: string;
  position_ms?: number;
  duration_ms?: number;
  volume?: number;
  shuffle?: boolean;
  repeat?: string;
  player_id?: string;
}

export interface PlayerInfo {
  id: string;
  name: string;
}

export interface Settings {
  trackpad_sensitivity: number;
  theme: string;
  app_shortcuts: AppShortcut[];
  visible_actions: string[];
  audio_device?: string;
}

export interface AppShortcut {
  id: string;
  name: string;
  command: string[];
  icon?: string;
}
