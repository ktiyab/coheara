/** L4-03: WiFi Transfer frontend types. */

export interface TransferSession {
  session_id: string;
  server_addr: string;
  url: string;
  pin: string;
  started_at: string;
  upload_count: number;
  max_uploads: number;
  timeout_secs: number;
}

export interface QrCodeData {
  url: string;
  pin: string;
  svg: string;
}

export interface UploadResult {
  filename: string;
  size_bytes: number;
  mime_type: string;
  received_at: string;
}

export interface TransferStatusResponse {
  session: TransferSession;
  received_files: UploadResult[];
}

export type TransferStatus = 'idle' | 'starting' | 'active' | 'stopping' | 'error';
