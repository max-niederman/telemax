<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '$lib/api.svelte';

  interface WindowLayout {
    pos_in_scrolling_layout?: [number, number];
    tile_size?: [number, number];
    window_size?: [number, number];
  }

  interface NiriWindow {
    id: number;
    title: string;
    app_id?: string;
    workspace_id?: number;
    is_focused?: boolean;
    layout?: WindowLayout;
  }

  interface NiriWorkspace {
    id: number;
    idx: number;
    name?: string;
    output?: string;
    is_active?: boolean;
    is_focused?: boolean;
    active_window_id?: number | null;
  }

  interface NiriOutput {
    logical?: { width: number; height: number; scale: number };
    model?: string;
  }

  let windows = $state<NiriWindow[]>([]);
  let workspaces = $state<NiriWorkspace[]>([]);
  let outputs = $state<Record<string, NiriOutput>>({});
  let selectedOutput = $state<string | null>(null);
  let containerWidth = $state(390);
  let containerEl: HTMLDivElement;
  let cleanups: (() => void)[] = [];

  let activeOutputs = $derived(
    Object.entries(outputs)
      .filter(([_, o]) => o.logical)
      .map(([name, o]) => ({ name, ...o }))
  );

  let multiMonitor = $derived(activeOutputs.length > 1);

  // Auto-select focused output
  $effect(() => {
    if (!selectedOutput && activeOutputs.length > 0) {
      const focusedWs = workspaces.find(ws => ws.is_focused);
      selectedOutput = focusedWs?.output ?? activeOutputs[0].name;
    }
  });

  // Workspaces for selected output, sorted, non-empty only
  let outputWorkspaces = $derived(
    workspaces
      .filter(ws => ws.output === selectedOutput)
      .filter(ws => windows.some(w => w.workspace_id === ws.id))
      .sort((a, b) => a.idx - b.idx)
  );

  // Monitor dimensions
  let monitorWidth = $derived(
    outputs[selectedOutput ?? '']?.logical?.width ?? 3840
  );
  let monitorHeight = $derived(
    outputs[selectedOutput ?? '']?.logical?.height ?? 2160
  );

  interface RenderedWindow {
    win: NiriWindow;
    x: number;
    y: number;
    w: number;
    h: number;
  }

  interface RenderedWorkspace {
    ws: NiriWorkspace;
    totalWidth: number;
    totalHeight: number;
    windows: RenderedWindow[];
    scrollOffset: number; // x offset to center the active window
  }

  // Build absolute-positioned layout for each workspace
  function buildLayout(wsId: number, activeWindowId: number | null | undefined): RenderedWorkspace | null {
    const ws = workspaces.find(w => w.id === wsId);
    if (!ws) return null;

    const wsWindows = windows.filter(w => w.workspace_id === wsId);
    if (wsWindows.length === 0) return null;

    // Group by column
    const columns = new Map<number, NiriWindow[]>();
    for (const w of wsWindows) {
      const col = w.layout?.pos_in_scrolling_layout?.[0] ?? 1;
      if (!columns.has(col)) columns.set(col, []);
      columns.get(col)!.push(w);
    }

    const sortedCols = [...columns.keys()].sort((a, b) => a - b);

    // Calculate column widths and x positions
    const colMeta: { col: number; x: number; width: number }[] = [];
    let xCursor = 0;
    for (const col of sortedCols) {
      const colWindows = columns.get(col)!;
      const width = Math.max(...colWindows.map(w => w.layout?.tile_size?.[0] ?? 800));
      colMeta.push({ col, x: xCursor, width });
      xCursor += width;
    }
    const totalWidth = xCursor;

    // Build rendered windows with absolute positions
    const rendered: RenderedWindow[] = [];
    let maxHeight = 0;

    for (const { col, x, width } of colMeta) {
      const colWindows = columns.get(col)!.sort(
        (a, b) => (a.layout?.pos_in_scrolling_layout?.[1] ?? 0) - (b.layout?.pos_in_scrolling_layout?.[1] ?? 0)
      );
      let yCursor = 0;
      for (const win of colWindows) {
        const h = win.layout?.tile_size?.[1] ?? monitorHeight;
        const w = win.layout?.tile_size?.[0] ?? width;
        rendered.push({ win, x, y: yCursor, w, h });
        yCursor += h;
      }
      maxHeight = Math.max(maxHeight, yCursor);
    }

    // Calculate scroll offset to center the active window's column
    let scrollOffset = 0;
    if (activeWindowId != null) {
      const activeWin = rendered.find(r => r.win.id === activeWindowId);
      if (activeWin) {
        // Center the active window's column in the monitor viewport
        const centerOfWindow = activeWin.x + activeWin.w / 2;
        scrollOffset = centerOfWindow - monitorWidth / 2;
        // Clamp so we don't go past the edges
        scrollOffset = Math.max(0, Math.min(scrollOffset, totalWidth - monitorWidth));
      }
    }

    return { ws, totalWidth, totalHeight: maxHeight, windows: rendered, scrollOffset };
  }

  let renderedWorkspaces = $derived(
    outputWorkspaces
      .map(ws => buildLayout(ws.id, ws.active_window_id))
      .filter((l): l is RenderedWorkspace => l !== null)
  );

  async function fetchAll() {
    try {
      const [w, ws, o] = await Promise.all([
        api.get<any>('/niri/windows'),
        api.get<any>('/niri/workspaces'),
        api.get<any>('/niri/outputs'),
      ]);
      if (Array.isArray(w)) windows = w;
      if (Array.isArray(ws)) workspaces = ws;
      if (o && typeof o === 'object' && !Array.isArray(o)) outputs = o;
    } catch {
      // ignore
    }
  }

  async function focusWindow(id: number) {
    api.post('/niri/action', { action: 'focus-window', args: { id } }).catch(() => {});
    setTimeout(fetchAll, 300);
  }

  onMount(() => {
    fetchAll();
    cleanups.push(api.on('niri_event', () => fetchAll()));
    // Track container width
    if (containerEl) {
      containerWidth = containerEl.clientWidth;
      const ro = new ResizeObserver(([entry]) => {
        containerWidth = entry.contentRect.width;
      });
      ro.observe(containerEl);
      cleanups.push(() => ro.disconnect());
    }
  });

  onDestroy(() => {
    cleanups.forEach(fn => fn());
  });
</script>

<div class="windows-page">
  {#if multiMonitor}
    <div class="output-selector">
      {#each activeOutputs as output}
        <button
          class="output-tab"
          class:active={selectedOutput === output.name}
          onclick={() => { selectedOutput = output.name; }}
        >
          {(output.model || output.name).toUpperCase()}
        </button>
      {/each}
    </div>
  {/if}

  <div class="workspaces-scroll" bind:this={containerEl}>
    {#each renderedWorkspaces as layout, wsIdx}
      {@const scale = containerWidth / monitorWidth}
      {@const viewportHeight = monitorHeight * scale}
      {@const isActive = layout.ws.is_active}
      <div class="workspace-container" class:active={isActive}>
        <div class="workspace-header">
          <span class="ws-idx">{layout.ws.name?.toUpperCase() || wsIdx + 1}</span>
          {#if isActive}<span class="active-marker"></span>{/if}
        </div>
        <div
          class="viewport"
          style="height: {viewportHeight}px"
        >
          <div
            class="layout-canvas"
            style="
              width: {layout.totalWidth * scale}px;
              height: {layout.totalHeight * scale}px;
              transform: translateX({-layout.scrollOffset * scale}px);
            "
          >
            {#each layout.windows as rw}
              <button
                class="win-tile"
                class:focused={rw.win.is_focused}
                class:active-win={rw.win.id === layout.ws.active_window_id}
                style="
                  left: {rw.x * scale}px;
                  top: {rw.y * scale}px;
                  width: {rw.w * scale}px;
                  height: {rw.h * scale}px;
                "
                onclick={() => focusWindow(rw.win.id)}
              >
                <span class="win-app">{(rw.win.app_id || '?').toUpperCase()}</span>
                <span class="win-title">{rw.win.title}</span>
              </button>
            {/each}
          </div>
        </div>
      </div>
    {/each}

    {#if renderedWorkspaces.length === 0}
      <div class="empty">NO WINDOWS</div>
    {/if}
  </div>
</div>

<style>
  .windows-page {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .output-selector {
    display: flex;
    flex-shrink: 0;
    border-bottom: 1px solid #333333;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .output-selector::-webkit-scrollbar { display: none; }

  .output-tab {
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
  }
  .output-tab.active {
    color: #ff2d2d;
    border-bottom-color: #ff2d2d;
  }

  .workspaces-scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    touch-action: pan-y;
    padding: 8px 0 24px;
  }

  .workspace-container {
    margin-bottom: 12px;
  }

  .workspace-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
  }

  .ws-idx {
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 10px;
    font-weight: 700;
    color: #444444;
    letter-spacing: 0.1em;
  }

  .workspace-container.active .ws-idx {
    color: #888888;
  }

  .active-marker {
    width: 4px;
    height: 4px;
    background: #ff2d2d;
  }

  .viewport {
    overflow: hidden;
    position: relative;
  }

  .layout-canvas {
    position: absolute;
    top: 0;
    left: 0;
  }

  .win-tile {
    position: absolute;
    border: none;
    outline: 1px solid #222222;
    background: #161616;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 3px 5px;
    overflow: hidden;
    text-align: left;
    box-sizing: border-box;
  }

  .win-tile.active-win {
    background: #1c1c1c;
    outline-color: #444444;
  }

  .win-tile.focused {
    background: #1c1c1c;
    outline: 1px solid #ff2d2d;
  }

  .win-tile:active {
    background: #252525;
  }

  .win-app {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 8px;
    font-weight: 700;
    color: #555555;
    letter-spacing: 0.06em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1.1;
  }

  .win-tile.active-win .win-app,
  .win-tile.focused .win-app {
    color: #999999;
  }

  .win-title {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 9px;
    font-weight: 400;
    color: #888888;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1.2;
  }

  .win-tile.active-win .win-title {
    color: #cccccc;
  }

  .win-tile.focused .win-title {
    color: #ffffff;
  }

  .empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.2em;
    color: #333333;
    padding-top: 40vh;
  }
</style>
