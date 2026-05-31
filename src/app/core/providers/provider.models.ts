export interface ProviderHealthDto {
  provider: string;
  healthy: boolean;
  base_url: string;
  message: string | null;
}

export interface ModelInfoDto {
  provider: string;
  id: string;
  display_name: string;
  size_bytes: number | null;
  modified_at: string | null;
}

export interface ProviderHealth {
  provider: string;
  healthy: boolean;
  baseUrl: string;
  message: string | null;
}

export interface ModelInfo {
  provider: string;
  id: string;
  displayName: string;
  sizeBytes: number | null;
  modifiedAt: string | null;
}
