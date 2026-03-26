<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api.svelte';
  import type { Settings } from '$lib/types';

  interface StatusInfo {
    tailscale_ip?: string;
    tailscale_name?: string;
    hostname?: string;
    connected?: boolean;
  }

  let settings = $state<Settings>({
    trackpad_sensitivity: 1.0,
    theme: 'dark',
    visible_actions: [],
  });
  let status = $state<StatusInfo>({});
  let saving = $state(false);
  let saveMessage = $state('');
  let dirty = $state(false);

  async function fetchSettings() {
    try {
      settings = await api.get<Settings>('/settings');
    } catch {
      // ignore
    }
  }

  async function fetchStatus() {
    try {
      status = await api.get<StatusInfo>('/status');
    } catch {
      // ignore
    }
  }

  async function save() {
    saving = true;
    saveMessage = '';
    try {
      await api.put('/settings', settings);
      saveMessage = 'Saved';
      dirty = false;
      setTimeout(() => { saveMessage = ''; }, 2000);
    } catch {
      saveMessage = 'Error saving';
    } finally {
      saving = false;
    }
  }

  function markDirty() {
    dirty = true;
  }

  function handleSensitivity(e: Event) {
    settings.trackpad_sensitivity = parseFloat((e.target as HTMLInputElement).value);
    markDirty();
  }

  function toggleTheme() {
    settings.theme = settings.theme === 'dark' ? 'light' : 'dark';
    markDirty();
  }

  onMount(() => {
    fetchSettings();
    fetchStatus();
  });
</script>

<div class="settings-page">
  <!-- Connection status -->
  <section class="setting-section">
    <div class="section-header">CONNECTION</div>
    <div class="setting-group">
      <div class="status-line">
        <span class="status-text" class:online={status.connected !== false}>
          {status.connected !== false ? 'CONNECTED' : 'DISCONNECTED'}
        </span>
      </div>
      {#if status.tailscale_name}
        <div class="detail-row">
          <span class="detail-key">IDENTITY</span>
          <span class="detail-val">{status.tailscale_name}</span>
        </div>
      {/if}
      {#if status.tailscale_ip}
        <div class="detail-row">
          <span class="detail-key">TAILSCALE IP</span>
          <span class="detail-val">{status.tailscale_ip}</span>
        </div>
      {/if}
      {#if status.hostname}
        <div class="detail-row">
          <span class="detail-key">HOST</span>
          <span class="detail-val">{status.hostname}</span>
        </div>
      {/if}
    </div>
  </section>

  <!-- Trackpad sensitivity -->
  <section class="setting-section">
    <div class="section-header">TRACKPAD</div>
    <div class="setting-group">
      <div class="setting-row">
        <span class="setting-name">SENSITIVITY</span>
        <span class="setting-value">{settings.trackpad_sensitivity.toFixed(1)}</span>
      </div>
      <input
        type="range"
        class="slider"
        min="0.1"
        max="3.0"
        step="0.1"
        value={settings.trackpad_sensitivity}
        oninput={handleSensitivity}
        aria-label="Trackpad sensitivity"
      />
    </div>
  </section>

  <!-- Save button -->
  <div class="save-area">
    <button class="save-btn" class:dirty onclick={save} disabled={saving}>
      {#if saving}
        SAVING...
      {:else if saveMessage}
        {saveMessage.toUpperCase()}
      {:else}
        SAVE SETTINGS
      {/if}
    </button>
  </div>
</div>

<style>
  .settings-page {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    touch-action: pan-y;
    padding: 0 0 24px;
    gap: 0;
  }

  .section-header {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    color: #666666;
    letter-spacing: 0.15em;
    padding: 20px 16px 8px;
    border-top: 1px solid #333333;
  }

  .setting-section:first-child .section-header {
    border-top: none;
  }

  .setting-section {
    flex-shrink: 0;
  }

  .setting-group {
    padding: 0 16px;
  }

  .status-line {
    padding: 8px 0;
  }

  .status-text {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: #666666;
  }

  .status-text.online {
    color: #ff2d2d;
  }

  .detail-row {
    display: flex;
    justify-content: space-between;
    padding: 8px 0;
    border-top: 1px solid #1a1a1a;
  }

  .detail-key {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.1em;
    color: #666666;
  }

  .detail-val {
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 13px;
    color: #ffffff;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 48px;
  }

  .setting-name {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: #ffffff;
  }

  .setting-value {
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 14px;
    color: #ff2d2d;
    font-weight: 500;
  }

  .slider {
    width: 100%;
    -webkit-appearance: none;
    appearance: none;
    height: 1px;
    background: #333333;
    outline: none;
    margin-top: 4px;
  }

  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    background: #ffffff;
    cursor: pointer;
  }

  .slider::-moz-range-thumb {
    width: 14px;
    height: 14px;
    background: #ffffff;
    border: none;
    cursor: pointer;
  }

  .toggle-text {
    background: transparent;
    border: none;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: #666666;
    cursor: pointer;
    padding: 8px 0;
  }

  .toggle-text.on {
    color: #ff2d2d;
  }

  .save-area {
    padding: 20px 16px;
    flex-shrink: 0;
  }

  .save-btn {
    width: 100%;
    padding: 16px;
    background: transparent;
    border: 1px solid #333333;
    color: #666666;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.15em;
    cursor: pointer;
  }

  .save-btn.dirty {
    border-color: #ff2d2d;
    color: #ff2d2d;
  }

  .save-btn:active {
    color: #ffffff;
  }

  .save-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
