export type ThemeMode = 'dark' | 'light';
export type LanguageCode = 'en' | 'id';

export interface FileItem {
  id: string;
  name: string;
  path: string;
  type: 'sol' | 'json' | 'prompt' | 'graphql' | 'ts';
  size: string;
  dirty?: boolean;
}

export interface ConsoleLog {
  id: string;
  timestamp: string;
  level: 'info' | 'success' | 'warn' | 'error' | 'receipt' | 'exec';
  tag: string;
  message: string;
  data?: Record<string, unknown> | string;
}

export interface AccountInfo {
  address: string;
  balanceEth: string;
  balanceUsd: string;
  nonce: number;
  keyType: string;
  status: 'active' | 'synced' | 'idle';
}

export interface NetworkTelemetry {
  chainId: number;
  networkName: string;
  rpcUrl: string;
  latencyMs: number;
  baseFeeGwei: number;
  priorityFeeGwei: number;
  memoryUsedMb: number;
  blockHeight: number;
}
