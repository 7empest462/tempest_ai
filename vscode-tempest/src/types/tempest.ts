/**
 * Copyright (c) 2026 Robert Simens. All Rights Reserved.
 * Licensed under the Tempest AI Source-Available License.
 * See LICENSE in the project root for full license information.
 */

// src/types/tempest.ts

export interface TempestChatParams {
  message: string;
  mode?: 'planning' | 'execution';
  backend?: 'mlx' | 'ollama';
}

export interface TempestRequest {
  method: 'tempest/chat' | 'tempest/status' | 'tempest/switch_backend' | 'tempest/clear_history' | 'tempest/get_state';
  params?: any;
}

export interface TempestChatChunk {
  content: string;
  reasoning?: string;
  is_streaming: boolean;
  done: boolean;
}

export interface TempestStatusResponse {
  method: 'tempest/status';
  backend: 'mlx' | 'ollama';
  phase: string;
  model: string;
  ram_usage_mb: number;
  context_tokens: number;
}

export interface TempestSwitchBackendResponse {
  method: 'tempest/switch_backend';
  success: boolean;
  message: string;
}

export interface TempestClearHistoryResponse {
  method: 'tempest/clear_history';
  success: boolean;
}

export interface TempestStateResponse {
  method: 'tempest/get_state';
  phase: string;
  planning_enabled: boolean;
  recent_tool_calls: string[];
}

// Union type for all possible responses
export type TempestResponse =
  | { method: 'tempest/chat'; payload: TempestChatChunk }
  | TempestStatusResponse
  | TempestSwitchBackendResponse
  | TempestClearHistoryResponse
  | TempestStateResponse;
