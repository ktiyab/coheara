/** ME-02: Device management types. */

export interface DeviceSummary {
  device_id: string;
  device_name: string;
  device_model: string;
  paired_at: string;
  last_seen: string;
  is_connected: boolean;
  has_websocket: boolean;
  days_inactive: number | null;
}

export interface DeviceCount {
  paired: number;
  connected: number;
  max: number;
}

export interface InactiveWarning {
  device_id: string;
  device_name: string;
  last_seen: string;
  days_inactive: number;
  message: string;
}
