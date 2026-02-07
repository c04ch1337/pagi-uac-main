/**
 * Ultimate route fix: the UI talks ONLY to the Rust Gateway.
 * No localhost:3001, no mock, no sandbox. Single source of truth.
 */
export const GATEWAY_ORIGIN = 'http://127.0.0.1:8001';
export const API_BASE_URL = `${GATEWAY_ORIGIN}/api/v1`;
export const GATEWAY_CHAT_URL = `${API_BASE_URL}/chat`;
