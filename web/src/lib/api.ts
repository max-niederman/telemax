import { base } from '$app/paths';

const WS_RECONNECT_BASE = 1000;
const WS_RECONNECT_MAX = 30000;

export type WsHandler = (data: any) => void;

export class ApiClient {
  private ws: WebSocket | null = null;
  private reconnectDelay = WS_RECONNECT_BASE;
  private listeners: Map<string, Set<WsHandler>> = new Map();
  private _connected = $state(false);

  get connected() { return this._connected; }

  async get<T>(path: string): Promise<T> {
    const res = await fetch(`${base}/api${path}`);
    if (!res.ok) throw await res.json();
    return res.json();
  }

  async post<T = void>(path: string, body?: unknown): Promise<T> {
    const res = await fetch(`${base}/api${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!res.ok) throw await res.json();
    if (res.status === 204) return undefined as T;
    return res.json();
  }

  async put<T = void>(path: string, body: unknown): Promise<T> {
    const res = await fetch(`${base}/api${path}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw await res.json();
    if (res.status === 204) return undefined as T;
    return res.json();
  }

  connectWs() {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    this.ws = new WebSocket(`${proto}//${location.host}${base}/api/ws`);
    this.ws.onopen = () => { this._connected = true; this.reconnectDelay = WS_RECONNECT_BASE; };
    this.ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      this.listeners.get(msg.type)?.forEach(h => h(msg));
    };
    this.ws.onclose = () => {
      this._connected = false;
      setTimeout(() => this.connectWs(), this.reconnectDelay);
      this.reconnectDelay = Math.min(this.reconnectDelay * 2, WS_RECONNECT_MAX);
    };
  }

  send(msg: unknown) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  on(type: string, handler: WsHandler): () => void {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(handler);
    return () => { this.listeners.get(type)?.delete(handler); };
  }
}

export const api = new ApiClient();
