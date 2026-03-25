<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api.svelte';
  import type { Settings, AppShortcut } from '$lib/types';

  const ALL_NIRI_ACTIONS = [
    { name: 'close-window', label: 'Close Window' },
    { name: 'fullscreen-window', label: 'Fullscreen Window' },
    { name: 'maximize-column', label: 'Maximize Column' },
    { name: 'toggle-window-floating', label: 'Toggle Floating' },
    { name: 'switch-preset-column-width', label: 'Switch Column Width' },
    { name: 'screenshot', label: 'Screenshot' },
    { name: 'screenshot-screen', label: 'Screenshot Screen' },
    { name: 'screenshot-window', label: 'Screenshot Window' },
    { name: 'power-off-monitors', label: 'Power Off Monitors' },
    { name: 'power-on-monitors', label: 'Power On Monitors' },
    { name: 'focus-monitor-left', label: 'Focus Monitor Left' },
    { name: 'focus-monitor-right', label: 'Focus Monitor Right' },
    { name: 'move-window-to-monitor-left', label: 'Move Window to Monitor Left' },
    { name: 'move-window-to-monitor-right', label: 'Move Window to Monitor Right' },
  ];

  interface StatusInfo {
    tailscale_ip?: string;
    tailscale_name?: string;
    hostname?: string;
    connected?: boolean;
  }

  let settings = $state<Settings>({
    trackpad_sensitivity: 1.0,
    theme: 'dark',
    app_shortcuts: [],
    visible_actions: [],
  });
  let status = $state<StatusInfo>({});
  let saving = $state(false);
  let saveMessage = $state('');
  let dirty = $state(false);

  // Inline form for adding app shortcuts
  let addingApp = $state(false);
  let newAppName = $state('');
  let newAppCommand = $state('');

  // Editing app shortcuts
  let editingAppId = $state<string | null>(null);
  let editAppName = $state('');
  let editAppCommand = $state('');

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

  function toggleAction(name: string) {
    const idx = settings.visible_actions.indexOf(name);
    if (idx >= 0) {
      settings.visible_actions = settings.visible_actions.filter((a) => a !== name);
    } else {
      settings.visible_actions = [...settings.visible_actions, name];
    }
    markDirty();
  }

  function addAppShortcut() {
    if (!newAppName.trim() || !newAppCommand.trim()) return;
    const shortcut: AppShortcut = {
      id: crypto.randomUUID(),
      name: newAppName.trim(),
      command: newAppCommand.trim().split(/\s+/),
    };
    settings.app_shortcuts = [...settings.app_shortcuts, shortcut];
    newAppName = '';
    newAppCommand = '';
    addingApp = false;
    markDirty();
  }

  function removeAppShortcut(id: string) {
    settings.app_shortcuts = settings.app_shortcuts.filter((a) => a.id !== id);
    markDirty();
  }

  function startEditApp(app: AppShortcut) {
    editingAppId = app.id;
    editAppName = app.name;
    editAppCommand = app.command.join(' ');
  }

  function saveEditApp() {
    if (!editingAppId || !editAppName.trim() || !editAppCommand.trim()) return;
    settings.app_shortcuts = settings.app_shortcuts.map((a) =>
      a.id === editingAppId
        ? { ...a, name: editAppName.trim(), command: editAppCommand.trim().split(/\s+/) }
        : a
    );
    editingAppId = null;
    markDirty();
  }

  function cancelEditApp() {
    editingAppId = null;
  }

  function handleAudioDevice(e: Event) {
    settings.audio_device = (e.target as HTMLInputElement).value || undefined;
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
    <div class="section-label">Connection</div>
    <div class="connection-card">
      <div class="status-row">
        <div class="status-dot" class:online={status.connected !== false}></div>
        <span class="status-text">
          {status.connected !== false ? 'Connected' : 'Disconnected'}
        </span>
      </div>
      {#if status.tailscale_name}
        <div class="status-detail">
          <span class="detail-label">Identity</span>
          <span class="detail-value">{status.tailscale_name}</span>
        </div>
      {/if}
      {#if status.tailscale_ip}
        <div class="status-detail">
          <span class="detail-label">Tailscale IP</span>
          <span class="detail-value">{status.tailscale_ip}</span>
        </div>
      {/if}
      {#if status.hostname}
        <div class="status-detail">
          <span class="detail-label">Host</span>
          <span class="detail-value">{status.hostname}</span>
        </div>
      {/if}
    </div>
  </section>

  <!-- Trackpad sensitivity -->
  <section class="setting-section">
    <div class="section-label">Trackpad</div>
    <div class="setting-card">
      <div class="setting-row">
        <span class="setting-name">Sensitivity</span>
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

  <!-- Theme -->
  <section class="setting-section">
    <div class="section-label">Appearance</div>
    <div class="setting-card">
      <div class="setting-row">
        <span class="setting-name">Theme</span>
        <button class="toggle-switch" class:on={settings.theme === 'dark'} onclick={toggleTheme} aria-label="Toggle theme">
          <div class="toggle-thumb"></div>
          <span class="toggle-label">{settings.theme === 'dark' ? 'Dark' : 'Light'}</span>
        </button>
      </div>
    </div>
  </section>

  <!-- Action buttons config -->
  <section class="setting-section">
    <div class="section-label">Visible Actions</div>
    <div class="setting-card">
      {#each ALL_NIRI_ACTIONS as action}
        <label class="checkbox-row">
          <input
            type="checkbox"
            checked={settings.visible_actions.includes(action.name)}
            onchange={() => toggleAction(action.name)}
          />
          <span class="checkbox-custom"></span>
          <span class="checkbox-label">{action.label}</span>
        </label>
      {/each}
    </div>
  </section>

  <!-- App shortcuts editor -->
  <section class="setting-section">
    <div class="section-label">App Shortcuts</div>
    <div class="setting-card">
      {#each settings.app_shortcuts as app}
        <div class="app-row">
          {#if editingAppId === app.id}
            <div class="app-edit-form">
              <input
                class="text-input"
                type="text"
                bind:value={editAppName}
                placeholder="Name"
              />
              <input
                class="text-input"
                type="text"
                bind:value={editAppCommand}
                placeholder="Command"
              />
              <div class="app-edit-actions">
                <button class="btn-sm save" onclick={saveEditApp}>Save</button>
                <button class="btn-sm" onclick={cancelEditApp}>Cancel</button>
              </div>
            </div>
          {:else}
            <div class="app-info">
              <span class="app-row-name">{app.name}</span>
              <span class="app-row-cmd">{app.command.join(' ')}</span>
            </div>
            <div class="app-actions">
              <button class="icon-btn" onclick={() => startEditApp(app)} aria-label="Edit">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                  <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                </svg>
              </button>
              <button class="icon-btn danger" onclick={() => removeAppShortcut(app.id)} aria-label="Delete">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="3,6 5,6 21,6" />
                  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                </svg>
              </button>
            </div>
          {/if}
        </div>
      {/each}

      {#if addingApp}
        <div class="app-add-form">
          <input
            class="text-input"
            type="text"
            bind:value={newAppName}
            placeholder="App name"
          />
          <input
            class="text-input"
            type="text"
            bind:value={newAppCommand}
            placeholder="Command (e.g. firefox)"
          />
          <div class="app-edit-actions">
            <button class="btn-sm save" onclick={addAppShortcut}>Add</button>
            <button class="btn-sm" onclick={() => { addingApp = false; }}>Cancel</button>
          </div>
        </div>
      {:else}
        <button class="add-btn" onclick={() => { addingApp = true; }}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
          Add Shortcut
        </button>
      {/if}
    </div>
  </section>

  <!-- Audio device -->
  <section class="setting-section">
    <div class="section-label">Audio</div>
    <div class="setting-card">
      <div class="setting-row">
        <span class="setting-name">Audio Device</span>
      </div>
      <input
        class="text-input full"
        type="text"
        value={settings.audio_device ?? ''}
        oninput={handleAudioDevice}
        placeholder="Default device"
      />
    </div>
  </section>

  <!-- Save button -->
  <div class="save-area">
    <button class="save-btn" class:dirty onclick={save} disabled={saving}>
      {#if saving}
        Saving...
      {:else if saveMessage}
        {saveMessage}
      {:else}
        Save Settings
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
    padding: 12px 0 24px;
    gap: 20px;
  }

  .section-label {
    font-size: 12px;
    font-weight: 600;
    color: #64748b;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0 16px;
    margin-bottom: 8px;
  }

  .setting-section {
    flex-shrink: 0;
  }

  .setting-card,
  .connection-card {
    background: #16213e;
    margin: 0 12px;
    border-radius: 12px;
    padding: 12px 16px;
    border: 1px solid #0f3460;
  }

  /* Connection */
  .status-row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }

  .status-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #ef4444;
    flex-shrink: 0;
  }

  .status-dot.online {
    background: #22c55e;
    box-shadow: 0 0 6px rgba(34, 197, 94, 0.4);
  }

  .status-text {
    font-size: 15px;
    font-weight: 600;
    color: #e2e8f0;
  }

  .status-detail {
    display: flex;
    justify-content: space-between;
    padding: 6px 0;
    border-top: 1px solid rgba(15, 52, 96, 0.5);
  }

  .detail-label {
    font-size: 13px;
    color: #64748b;
  }

  .detail-value {
    font-size: 13px;
    color: #94a3b8;
    font-family: monospace;
  }

  /* Setting rows */
  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 44px;
  }

  .setting-name {
    font-size: 15px;
    color: #e2e8f0;
  }

  .setting-value {
    font-size: 14px;
    color: #7c3aed;
    font-weight: 600;
    font-family: monospace;
  }

  /* Slider */
  .slider {
    width: 100%;
    -webkit-appearance: none;
    appearance: none;
    height: 4px;
    background: #0f3460;
    border-radius: 2px;
    outline: none;
    margin-top: 4px;
  }

  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 22px;
    height: 22px;
    background: #7c3aed;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 2px 6px rgba(124, 58, 237, 0.3);
  }

  .slider::-moz-range-thumb {
    width: 22px;
    height: 22px;
    background: #7c3aed;
    border: none;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 2px 6px rgba(124, 58, 237, 0.3);
  }

  /* Toggle switch */
  .toggle-switch {
    position: relative;
    width: 52px;
    height: 30px;
    background: #0f3460;
    border-radius: 15px;
    border: none;
    cursor: pointer;
    transition: background 0.2s;
    display: flex;
    align-items: center;
    padding: 0;
  }

  .toggle-switch.on {
    background: #7c3aed;
  }

  .toggle-thumb {
    width: 24px;
    height: 24px;
    background: #e2e8f0;
    border-radius: 50%;
    position: absolute;
    left: 3px;
    transition: transform 0.2s;
  }

  .toggle-switch.on .toggle-thumb {
    transform: translateX(22px);
  }

  .toggle-label {
    position: absolute;
    right: -48px;
    font-size: 13px;
    color: #94a3b8;
    white-space: nowrap;
  }

  /* Checkbox */
  .checkbox-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    cursor: pointer;
    border-bottom: 1px solid rgba(15, 52, 96, 0.3);
    min-height: 44px;
  }

  .checkbox-row:last-of-type {
    border-bottom: none;
  }

  .checkbox-row input[type="checkbox"] {
    display: none;
  }

  .checkbox-custom {
    width: 22px;
    height: 22px;
    border: 2px solid #0f3460;
    border-radius: 6px;
    flex-shrink: 0;
    position: relative;
    transition: all 0.15s;
  }

  .checkbox-row input:checked + .checkbox-custom {
    background: #7c3aed;
    border-color: #7c3aed;
  }

  .checkbox-row input:checked + .checkbox-custom::after {
    content: '';
    position: absolute;
    left: 6px;
    top: 2px;
    width: 6px;
    height: 11px;
    border: solid white;
    border-width: 0 2px 2px 0;
    transform: rotate(45deg);
  }

  .checkbox-label {
    font-size: 14px;
    color: #e2e8f0;
  }

  /* App shortcuts */
  .app-row {
    padding: 10px 0;
    border-bottom: 1px solid rgba(15, 52, 96, 0.3);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .app-row:last-of-type {
    border-bottom: none;
  }

  .app-info {
    flex: 1;
    min-width: 0;
  }

  .app-row-name {
    display: block;
    font-size: 14px;
    color: #e2e8f0;
    font-weight: 500;
  }

  .app-row-cmd {
    display: block;
    font-size: 12px;
    color: #64748b;
    font-family: monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .app-actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }

  .icon-btn {
    width: 36px;
    height: 36px;
    border: none;
    background: transparent;
    color: #64748b;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    transition: all 0.15s;
  }

  .icon-btn:active {
    background: rgba(124, 58, 237, 0.1);
  }

  .icon-btn.danger:active {
    background: rgba(239, 68, 68, 0.1);
    color: #ef4444;
  }

  .icon-btn svg {
    width: 18px;
    height: 18px;
  }

  .app-edit-form,
  .app-add-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 0;
    width: 100%;
  }

  .app-edit-actions {
    display: flex;
    gap: 8px;
  }

  .text-input {
    width: 100%;
    padding: 10px 12px;
    background: #1a1a2e;
    border: 1px solid #0f3460;
    border-radius: 8px;
    color: #e2e8f0;
    font-size: 14px;
    outline: none;
    transition: border-color 0.2s;
  }

  .text-input:focus {
    border-color: #7c3aed;
  }

  .text-input.full {
    margin-top: 8px;
  }

  .btn-sm {
    padding: 8px 16px;
    border: 1px solid #0f3460;
    background: #1a1a2e;
    color: #94a3b8;
    border-radius: 8px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn-sm.save {
    background: #7c3aed;
    border-color: #7c3aed;
    color: white;
  }

  .btn-sm:active {
    transform: scale(0.95);
  }

  .add-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    padding: 12px;
    background: transparent;
    border: 1px dashed #0f3460;
    border-radius: 8px;
    color: #64748b;
    font-size: 14px;
    cursor: pointer;
    margin-top: 8px;
    transition: all 0.15s;
  }

  .add-btn:active {
    background: rgba(124, 58, 237, 0.05);
    border-color: #7c3aed;
    color: #7c3aed;
  }

  .add-btn svg {
    width: 18px;
    height: 18px;
  }

  /* Save button */
  .save-area {
    padding: 12px 16px;
    flex-shrink: 0;
  }

  .save-btn {
    width: 100%;
    padding: 14px;
    background: #16213e;
    border: 1px solid #0f3460;
    border-radius: 12px;
    color: #94a3b8;
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .save-btn.dirty {
    background: #7c3aed;
    border-color: #7c3aed;
    color: white;
    box-shadow: 0 4px 12px rgba(124, 58, 237, 0.3);
  }

  .save-btn:active {
    transform: scale(0.98);
  }

  .save-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
