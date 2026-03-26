<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '$lib/api.svelte';

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

  const ALL_ACTIONS: { name: string; label: string }[] = [
    { name: 'screenshot', label: 'Screenshot' },
    { name: 'fullscreen-window', label: 'Fullscreen' },
    { name: 'maximize-column', label: 'Maximize' },
    { name: 'toggle-window-floating', label: 'Float' },
    { name: 'power-off-monitors', label: 'Monitors Off' },
    { name: 'power-on-monitors', label: 'Monitors On' },
    { name: 'close-window', label: 'Close Window' },
    { name: 'switch-preset-column-width', label: 'Column Width' },
    { name: 'focus-monitor-left', label: 'Mon Left' },
    { name: 'focus-monitor-right', label: 'Mon Right' },
    { name: 'move-window-to-monitor-left', label: 'Move Left' },
    { name: 'move-window-to-monitor-right', label: 'Move Right' },
  ];

  let windows = $state<NiriWindow[]>([]);
  let workspaces = $state<NiriWorkspace[]>([]);
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

  let visibleActions = ALL_ACTIONS;

  async function fetchAll() {
    try {
      const [w, ws] = await Promise.all([
        api.get<NiriWindow[]>('/niri/windows'),
        api.get<NiriWorkspace[]>('/niri/workspaces'),
      ]);
      windows = w;
      workspaces = ws;
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
          class="workspace-tab"
          class:active={ws.is_active}
          onclick={() => focusWorkspace(ws.idx)}
        >
          {(ws.name || `${ws.idx}`).toUpperCase()}
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
          <div class="ws-group-label">{(ws.name || `Workspace ${ws.idx}`).toUpperCase()}</div>
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
                <span class="window-app">{(win.app_id || 'Unknown').toUpperCase()}</span>
                <span class="window-title">{win.title}</span>
              </button>
              {#if swipedWindowId === win.id}
                <button class="window-close" onclick={() => closeWindow(win.id)} aria-label="Close window">
                  CLOSE
                </button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    {/each}
    {#if windows.length === 0}
      <div class="empty-state">NO WINDOWS OPEN</div>
    {/if}
  </section>

  <!-- Action button grid -->
  {#if visibleActions.length > 0}
    <section class="actions-section">
      <div class="section-label">ACTIONS</div>
      <div class="action-grid">
        {#each visibleActions as action}
          <button class="action-btn" onclick={() => runAction(action.name)}>
            {action.label.toUpperCase()}
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
    gap: 0;
    padding: 0;
  }

  .section-label {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    color: #666666;
    letter-spacing: 0.15em;
    padding: 16px 16px 8px;
    border-top: 1px solid #333333;
  }

  /* Workspace switcher */
  .workspace-section {
    flex-shrink: 0;
    border-bottom: 1px solid #333333;
  }

  .workspace-strip {
    display: flex;
    gap: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .workspace-strip::-webkit-scrollbar {
    display: none;
  }

  .workspace-tab {
    padding: 14px 20px;
    border: none;
    background: transparent;
    color: #666666;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    white-space: nowrap;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    min-width: 48px;
    text-align: center;
  }

  .workspace-tab.active {
    color: #ff2d2d;
    border-bottom-color: #ff2d2d;
  }

  /* Window list */
  .window-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .ws-group {
    margin-bottom: 0;
  }

  .ws-group-label {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    color: #333333;
    letter-spacing: 0.15em;
    padding: 12px 16px 4px;
  }

  .window-item {
    display: flex;
    position: relative;
    overflow: hidden;
    border-bottom: 1px solid #1a1a1a;
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
    width: 100%;
  }

  .window-content:active {
    color: #ff2d2d;
  }

  .window-item.focused .window-content {
    border-left: 2px solid #ff2d2d;
  }

  .window-app {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    color: #666666;
    letter-spacing: 0.1em;
  }

  .window-title {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 14px;
    font-weight: 300;
    color: #ffffff;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .window-close {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: 72px;
    background: #ff2d2d;
    border: none;
    color: #ffffff;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: slideIn 0.15s ease-out;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
  }

  @keyframes slideIn {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }

  .empty-state {
    text-align: center;
    color: #333333;
    padding: 48px 16px;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.2em;
  }

  /* Action grid */
  .actions-section {
    flex-shrink: 0;
  }

  .action-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0;
    padding: 0 16px 16px;
  }

  .action-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px 8px;
    background: transparent;
    border: 1px solid #333333;
    color: #ffffff;
    cursor: pointer;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.1em;
    text-align: center;
    min-height: 48px;
    margin-top: -1px;
    margin-left: -1px;
  }

  .action-btn:active {
    color: #ff2d2d;
  }

</style>
