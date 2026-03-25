<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '$lib/api';
  import type { Settings, AppShortcut } from '$lib/types';

  interface NiriWindow {
    id: number;
    title: string;
    app_id?: string;
    workspace_id?: number;
    is_focused?: boolean;
  }

  interface NiriWorkspace {
    id: number;
    idx: number;
    name?: string;
    is_active?: boolean;
    output?: string;
  }

  const ALL_ACTIONS: { name: string; label: string; icon: string }[] = [
    { name: 'screenshot', label: 'Screenshot', icon: 'camera' },
    { name: 'screenshot-screen', label: 'Screen Shot', icon: 'monitor' },
    { name: 'screenshot-window', label: 'Window Shot', icon: 'square' },
    { name: 'fullscreen-window', label: 'Fullscreen', icon: 'maximize' },
    { name: 'maximize-column', label: 'Maximize', icon: 'expand' },
    { name: 'toggle-window-floating', label: 'Float', icon: 'layers' },
    { name: 'power-off-monitors', label: 'Monitors Off', icon: 'power-off' },
    { name: 'power-on-monitors', label: 'Monitors On', icon: 'power-on' },
    { name: 'close-window', label: 'Close Window', icon: 'x' },
    { name: 'switch-preset-column-width', label: 'Column Width', icon: 'columns' },
    { name: 'focus-monitor-left', label: 'Mon Left', icon: 'arrow-left' },
    { name: 'focus-monitor-right', label: 'Mon Right', icon: 'arrow-right' },
    { name: 'move-window-to-monitor-left', label: 'Move Left', icon: 'move-left' },
    { name: 'move-window-to-monitor-right', label: 'Move Right', icon: 'move-right' },
  ];

  let windows = $state<NiriWindow[]>([]);
  let workspaces = $state<NiriWorkspace[]>([]);
  let settings = $state<Settings | null>(null);
  let apps = $state<AppShortcut[]>([]);
  let swipedWindowId = $state<number | null>(null);
  let cleanups: (() => void)[] = [];

  let sortedWorkspaces = $derived(
    [...workspaces].sort((a, b) => a.idx - b.idx)
  );

  let windowsByWorkspace = $derived(() => {
    const map = new Map<number, NiriWindow[]>();
    for (const w of windows) {
      const wsId = w.workspace_id ?? 0;
      if (!map.has(wsId)) map.set(wsId, []);
      map.get(wsId)!.push(w);
    }
    return map;
  });

  let visibleActions = $derived(
    ALL_ACTIONS.filter((a) =>
      settings?.visible_actions?.includes(a.name) ?? true
    )
  );

  async function fetchAll() {
    try {
      const [w, ws, s, a] = await Promise.all([
        api.get<NiriWindow[]>('/niri/windows'),
        api.get<NiriWorkspace[]>('/niri/workspaces'),
        api.get<Settings>('/settings'),
        api.get<AppShortcut[]>('/apps'),
      ]);
      windows = w;
      workspaces = ws;
      settings = s;
      apps = a;
    } catch {
      // ignore
    }
  }

  async function focusWorkspace(idx: number) {
    try {
      await api.post('/niri/action', { action: 'focus-workspace', args: [String(idx)] });
    } catch {
      // ignore
    }
  }

  async function focusWindow(id: number) {
    try {
      await api.post('/niri/action', { action: 'focus-window', args: [String(id)] });
    } catch {
      // ignore
    }
  }

  async function closeWindow(id: number) {
    swipedWindowId = null;
    try {
      await api.post('/niri/action', { action: 'close-window', args: [String(id)] });
      windows = windows.filter((w) => w.id !== id);
    } catch {
      // ignore
    }
  }

  async function runAction(action: string) {
    try {
      await api.post('/niri/action', { action });
    } catch {
      // ignore
    }
  }

  async function launchApp(shortcut: AppShortcut) {
    try {
      await api.post('/apps/launch', { id: shortcut.id });
    } catch {
      // ignore
    }
  }

  // Swipe handling for window items
  let touchStartX = 0;

  function handleWindowTouchStart(e: TouchEvent, id: number) {
    touchStartX = e.touches[0].clientX;
  }

  function handleWindowTouchEnd(e: TouchEvent, id: number) {
    const dx = e.changedTouches[0].clientX - touchStartX;
    if (dx < -60) {
      swipedWindowId = id;
    } else {
      if (swipedWindowId === id) {
        swipedWindowId = null;
      }
    }
  }

  onMount(() => {
    fetchAll();

    cleanups.push(
      api.on('niri_event', (_msg) => {
        // Re-fetch on any niri event for simplicity
        fetchAll();
      })
    );
  });

  onDestroy(() => {
    cleanups.forEach((fn) => fn());
  });
</script>

<div class="windows-page">
  <!-- Workspace switcher -->
  <section class="workspace-section">
    <div class="workspace-strip">
      {#each sortedWorkspaces as ws}
        <button
          class="workspace-pill"
          class:active={ws.is_active}
          onclick={() => focusWorkspace(ws.idx)}
        >
          {ws.name || `${ws.idx}`}
        </button>
      {/each}
    </div>
  </section>

  <!-- Window list -->
  <section class="window-list">
    {#each sortedWorkspaces as ws}
      {@const wsWindows = windowsByWorkspace().get(ws.id) ?? []}
      {#if wsWindows.length > 0}
        <div class="ws-group">
          <div class="ws-group-label">{ws.name || `Workspace ${ws.idx}`}</div>
          {#each wsWindows as win}
            <div
              class="window-item"
              class:focused={win.is_focused}
              class:swiped={swipedWindowId === win.id}
              role="listitem"
              ontouchstart={(e: TouchEvent) => handleWindowTouchStart(e, win.id)}
              ontouchend={(e: TouchEvent) => handleWindowTouchEnd(e, win.id)}
            >
              <button class="window-content" onclick={() => focusWindow(win.id)}>
                <span class="window-app">{win.app_id || 'Unknown'}</span>
                <span class="window-title">{win.title}</span>
              </button>
              {#if swipedWindowId === win.id}
                <button class="window-close" onclick={() => closeWindow(win.id)} aria-label="Close window">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                  </svg>
                </button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    {/each}
    {#if windows.length === 0}
      <div class="empty-state">No windows open</div>
    {/if}
  </section>

  <!-- Action button grid -->
  {#if visibleActions.length > 0}
    <section class="actions-section">
      <div class="section-label">Actions</div>
      <div class="action-grid">
        {#each visibleActions as action}
          <button class="action-btn" onclick={() => runAction(action.name)}>
            <svg class="action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              {#if action.icon === 'camera'}
                <path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z" />
                <circle cx="12" cy="13" r="4" />
              {:else if action.icon === 'monitor'}
                <rect x="2" y="3" width="20" height="14" rx="2" />
                <line x1="8" y1="21" x2="16" y2="21" />
                <line x1="12" y1="17" x2="12" y2="21" />
              {:else if action.icon === 'square'}
                <rect x="3" y="3" width="18" height="18" rx="2" />
              {:else if action.icon === 'maximize'}
                <polyline points="15,3 21,3 21,9" />
                <polyline points="9,21 3,21 3,15" />
                <line x1="21" y1="3" x2="14" y2="10" />
                <line x1="3" y1="21" x2="10" y2="14" />
              {:else if action.icon === 'expand'}
                <polyline points="15,3 21,3 21,9" />
                <polyline points="9,21 3,21 3,15" />
                <polyline points="21,15 21,21 15,21" />
                <polyline points="3,9 3,3 9,3" />
              {:else if action.icon === 'layers'}
                <polygon points="12,2 2,7 12,12 22,7" />
                <polyline points="2,17 12,22 22,17" />
                <polyline points="2,12 12,17 22,12" />
              {:else if action.icon === 'power-off'}
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="2" x2="12" y2="12" />
              {:else if action.icon === 'power-on'}
                <circle cx="12" cy="12" r="10" />
                <polyline points="8,12 12,8 16,12" />
              {:else if action.icon === 'x'}
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              {:else if action.icon === 'columns'}
                <rect x="3" y="3" width="7" height="18" rx="1" />
                <rect x="14" y="3" width="7" height="18" rx="1" />
              {:else if action.icon === 'arrow-left'}
                <line x1="19" y1="12" x2="5" y2="12" />
                <polyline points="12,19 5,12 12,5" />
              {:else if action.icon === 'arrow-right'}
                <line x1="5" y1="12" x2="19" y2="12" />
                <polyline points="12,5 19,12 12,19" />
              {:else if action.icon === 'move-left'}
                <polyline points="11,17 6,12 11,7" />
                <line x1="6" y1="12" x2="18" y2="12" />
                <line x1="3" y1="4" x2="3" y2="20" />
              {:else if action.icon === 'move-right'}
                <polyline points="13,7 18,12 13,17" />
                <line x1="6" y1="12" x2="18" y2="12" />
                <line x1="21" y1="4" x2="21" y2="20" />
              {:else}
                <circle cx="12" cy="12" r="10" />
              {/if}
            </svg>
            <span class="action-label">{action.label}</span>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  <!-- App launcher -->
  {#if apps.length > 0}
    <section class="apps-section">
      <div class="section-label">Apps</div>
      <div class="app-grid">
        {#each apps as app}
          <button class="app-btn" onclick={() => launchApp(app)}>
            {#if app.icon}
              <img src={app.icon} alt="" class="app-icon-img" />
            {:else}
              <div class="app-icon-placeholder">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="3" y="3" width="18" height="18" rx="4" />
                  <circle cx="12" cy="12" r="3" />
                </svg>
              </div>
            {/if}
            <span class="app-name">{app.name}</span>
          </button>
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .windows-page {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    touch-action: pan-y;
    gap: 16px;
    padding: 12px 0;
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

  /* Workspace switcher */
  .workspace-section {
    flex-shrink: 0;
    padding: 0 12px;
  }

  .workspace-strip {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 4px 4px;
    scrollbar-width: none;
  }

  .workspace-strip::-webkit-scrollbar {
    display: none;
  }

  .workspace-pill {
    padding: 8px 20px;
    border-radius: 20px;
    border: 1px solid #0f3460;
    background: #16213e;
    color: #94a3b8;
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
    cursor: pointer;
    transition: all 0.2s;
    flex-shrink: 0;
    min-width: 44px;
    text-align: center;
  }

  .workspace-pill.active {
    background: #7c3aed;
    border-color: #7c3aed;
    color: #fff;
    box-shadow: 0 2px 8px rgba(124, 58, 237, 0.3);
  }

  .workspace-pill:active {
    transform: scale(0.95);
  }

  /* Window list */
  .window-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .ws-group {
    margin-bottom: 12px;
  }

  .ws-group-label {
    font-size: 11px;
    font-weight: 600;
    color: #475569;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0 16px;
    margin-bottom: 4px;
  }

  .window-item {
    display: flex;
    position: relative;
    overflow: hidden;
    transition: transform 0.2s;
  }

  .window-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 12px 16px;
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    text-align: left;
    min-height: 48px;
    justify-content: center;
    transition: background 0.15s;
    width: 100%;
  }

  .window-content:active {
    background: rgba(124, 58, 237, 0.1);
  }

  .window-item.focused .window-content {
    border-left: 3px solid #7c3aed;
  }

  .window-app {
    font-size: 11px;
    color: #64748b;
    font-weight: 500;
  }

  .window-title {
    font-size: 14px;
    color: #e2e8f0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .window-close {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: 64px;
    background: #ef4444;
    border: none;
    color: white;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: slideIn 0.15s ease-out;
  }

  @keyframes slideIn {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }

  .window-close svg {
    width: 20px;
    height: 20px;
  }

  .empty-state {
    text-align: center;
    color: #475569;
    padding: 40px 16px;
    font-size: 15px;
  }

  /* Action grid */
  .actions-section {
    flex-shrink: 0;
  }

  .action-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    padding: 0 16px;
  }

  .action-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 14px 8px;
    background: #16213e;
    border: 1px solid #0f3460;
    border-radius: 12px;
    color: #e2e8f0;
    cursor: pointer;
    transition: all 0.15s;
    min-height: 72px;
  }

  .action-btn:active {
    background: #0f3460;
    transform: scale(0.95);
  }

  .action-icon {
    width: 22px;
    height: 22px;
  }

  .action-label {
    font-size: 11px;
    font-weight: 500;
    text-align: center;
    line-height: 1.2;
  }

  /* App launcher */
  .apps-section {
    flex-shrink: 0;
    padding-bottom: 12px;
  }

  .app-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
    padding: 0 16px;
  }

  .app-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 12px 4px;
    background: transparent;
    border: none;
    color: #e2e8f0;
    cursor: pointer;
    border-radius: 12px;
    transition: all 0.15s;
  }

  .app-btn:active {
    background: rgba(124, 58, 237, 0.1);
    transform: scale(0.95);
  }

  .app-icon-img {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    object-fit: cover;
  }

  .app-icon-placeholder {
    width: 44px;
    height: 44px;
    background: #16213e;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #475569;
  }

  .app-icon-placeholder svg {
    width: 24px;
    height: 24px;
  }

  .app-name {
    font-size: 11px;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
</style>
