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
  visible_actions: string[];
  audio_device?: string;
}
