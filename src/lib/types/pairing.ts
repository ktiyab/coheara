/** M0-02: Device pairing types. */

export interface QrPairingData {
  v: number;
  url: string;
  token: string;
  cert_fp: string;
  pubkey: string;
}

export interface PairingStartResponse {
  qr_svg: string;
  qr_data: QrPairingData;
  expires_at: string;
}

export interface PendingApproval {
  device_name: string;
  device_model: string;
}
