/**
 * Copyright (c) 2026 Robert Simens. All Rights Reserved.
 * Licensed under the Tempest AI Source-Available License.
 * See LICENSE in the project root for full license information.
 */

// src/utils/tempestBridge.ts
import * as vscode from 'vscode';
import { TempestRequest, TempestResponse, TempestChatChunk } from '../types/tempest';

declare const acquireVsCodeApi: any;

export class TempestBridge {
  private static instance: TempestBridge;
  private messageId = 0;
  private callbacks = new Map<number, (response: TempestResponse) => void>();
  private streamListeners = new Map<number, (chunk: TempestChatChunk) => void>();
  private vscodeApi: any;

  constructor() {
    try {
        this.vscodeApi = acquireVsCodeApi();
    } catch (e) {
        // Not in webview context
    }
  }

  static getInstance(): TempestBridge {
    if (!TempestBridge.instance) {
      TempestBridge.instance = new TempestBridge();
    }
    return TempestBridge.instance;
  }

  /**
   * Send a request and get a promise for the final result
   */
  async sendRequest<T extends TempestResponse>(request: TempestRequest): Promise<T> {
    const id = ++this.messageId;

    return new Promise((resolve, reject) => {
      this.callbacks.set(id, (response) => {
        this.callbacks.delete(id);
        this.streamListeners.delete(id);
        resolve(response as T);
      });

      this.vscodeApi.postMessage({
        type: 'tempest-request',
        id,
        payload: request
      });
    });
  }

  /**
   * Stream a chat message with real-time updates
   */
  async streamChat(
    message: string,
    mode: 'planning' | 'execution' = 'planning',
    backend: 'mlx' | 'ollama' = 'ollama',
    onChunk: (chunk: TempestChatChunk) => void
  ): Promise<TempestChatChunk> {
    const id = ++this.messageId;

    this.streamListeners.set(id, onChunk);

    return new Promise((resolve, reject) => {
      this.callbacks.set(id, (finalResponse: any) => {
        this.callbacks.delete(id);
        this.streamListeners.delete(id);
        resolve(finalResponse.payload);
      });

      this.vscodeApi.postMessage({
        type: 'tempest-request',
        id,
        payload: {
          method: 'tempest/chat',
          params: { message, mode, backend }
        }
      });
    });
  }

  /**
   * Called by sidebar HTML when it receives data from extension.ts
   */
  handleResponse(message: any) {
    if (message.type !== 'tempest-response' || !message.id) return;

    const { id, payload } = message;

    // Handle streaming chunks
    if (payload.method === 'tempest/chat') {
      const listener = this.streamListeners.get(id);
      if (listener) {
        listener(payload.payload as TempestChatChunk);
      }
    }

    // Handle final response
    const callback = this.callbacks.get(id);
    if (callback) {
      if (payload.method === 'tempest/chat' && !payload.payload.done) {
          // Don't resolve the final promise until done: true
          return;
      }
      callback(payload);
    }
  }
}
