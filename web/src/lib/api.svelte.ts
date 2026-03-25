import { base } from '$app/paths';

const WS_RECONNECT_BASE = 1000;
const WS_RECONNECT_MAX = 30000;
const TOKEN_KEY = 'telemax_session';

export type WsHandler = (data: any) => void;

export class ApiClient {
  private ws: WebSocket | null = null;
  private reconnectDelay = WS_RECONNECT_BASE;
  private listeners: Map<string, Set<WsHandler>> = new Map();
  private _connected = $state(false);
  private _needsPairing = $state(false);
  private _paired = $state(false);

  get connected() { return this._connected; }
  get needsPairing() { return this._needsPairing; }
  set needsPairing(v: boolean) { this._needsPairing = v; }
  get paired() { return this._paired; }

  constructor() {
    this._paired = !!this.getToken();
  }

  private getToken(): string | null {
    if (typeof localStorage === 'undefined') return null;
    return localStorage.getItem(TOKEN_KEY);
  }

  private setToken(token: string) {
    localStorage.setItem(TOKEN_KEY, token);
    this._paired = true;
    this._needsPairing = false;
  }

  private clearToken() {
    localStorage.removeItem(TOKEN_KEY);
    this._paired = false;
    this._needsPairing = true;
  }

  private authHeaders(): Record<string, string> {
    const token = this.getToken();
    if (token) {
      return { 'Authorization': `Bearer ${token}` };
    }
    return {};
  }

  private async handleResponse(res: Response): Promise<Response> {
    if (res.status === 403) {
      try {
        const body = await res.clone().json();
        if (body?.error?.code === 'AUTH_REQUIRED') {
          this.clearToken();
        }
      } catch {
        // not JSON, still treat as auth error
        this.clearToken();
      }
      throw await res.json();
    }
    return res;
  }

  async get<T>(path: string): Promise<T> {
    const res = await fetch(`${base}/api${path}`, {
      headers: { ...this.authHeaders() },
    });
    await this.handleResponse(res);
    if (!res.ok) throw await res.json();
    return res.json();
  }

  async post<T = void>(path: string, body?: unknown): Promise<T> {
    const res = await fetch(`${base}/api${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...this.authHeaders() },
      body: body ? JSON.stringify(body) : undefined,
    });
    await this.handleResponse(res);
    if (!res.ok) throw await res.json();
    if (res.status === 204) return undefined as T;
    return res.json();
  }

  async put<T = void>(path: string, body: unknown): Promise<T> {
    const res = await fetch(`${base}/api${path}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json', ...this.authHeaders() },
      body: JSON.stringify(body),
    });
    await this.handleResponse(res);
    if (!res.ok) throw await res.json();
    if (res.status === 204) return undefined as T;
    return res.json();
  }

  async pair(code: string): Promise<boolean> {
    const res = await fetch(`${base}/api/pair`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ code }),
    });
    if (!res.ok) return false;
    const data = await res.json();
    if (data.token) {
      this.setToken(data.token);
      return true;
    }
    return false;
  }

  async checkAuth(): Promise<void> {
    if (!this.getToken()) {
      this._needsPairing = true;
      return;
    }
    try {
      await this.get('/status');
    } catch {
      // handleResponse will have set needsPairing if 403
    }
  }

  connectWs() {
    const token = this.getToken();
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const params = token ? `?token=${encodeURIComponent(token)}` : '';
    this.ws = new WebSocket(`${proto}//${location.host}${base}/api/ws${params}`);
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
