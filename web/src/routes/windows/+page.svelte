<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '$lib/api.svelte';

  interface WindowLayout {
    pos_in_scrolling_layout?: [number, number];
    tile_size?: [number, number];
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

  // Horizontal pan state — shared across all workspaces
  let panX = $state(0); // current offset in scaled px (negative = scrolled right)
  let panStartX = 0;
  let panStartOffset = 0;
  let isPanning = false;
  let panVelocity = 0;
  let lastPanX = 0;
  let lastPanTime = 0;
  let momentumRaf: number | null = null;

  let activeOutputs = $derived(
    Object.entries(outputs)
      .filter(([_, o]) => o.logical)
      .map(([name, o]) => ({ name, ...o }))
  );

  let multiMonitor = $derived(activeOutputs.length > 1);

  $effect(() => {
    if (!selectedOutput && activeOutputs.length > 0) {
      const focusedWs = workspaces.find(ws => ws.is_focused);
      selectedOutput = focusedWs?.output ?? activeOutputs[0].name;
    }
  });

  let outputWorkspaces = $derived(
    workspaces
      .filter(ws => ws.output === selectedOutput)
      .filter(ws => windows.some(w => w.workspace_id === ws.id))
      .sort((a, b) => a.idx - b.idx)
  );

  let monitorWidth = $derived(outputs[selectedOutput ?? '']?.logical?.width ?? 3840);
  let monitorHeight = $derived(outputs[selectedOutput ?? '']?.logical?.height ?? 2160);
  let scale = $derived(containerWidth / monitorWidth);

  interface RenderedWindow {
    win: NiriWindow;
    x: number; y: number; w: number; h: number;
  }

  interface RenderedWorkspace {
    ws: NiriWorkspace;
    totalWidth: number;
    totalHeight: number;
    windows: RenderedWindow[];
  }

  function buildLayouts(): { layouts: RenderedWorkspace[]; maxTotalWidth: number; initialPanX: number } {
    const layouts: RenderedWorkspace[] = [];
    let maxTotalWidth = monitorWidth;
    let initialPanX = 0;

    for (const ws of outputWorkspaces) {
      const wsWindows = windows.filter(w => w.workspace_id === ws.id);
      if (wsWindows.length === 0) continue;

      const columns = new Map<number, NiriWindow[]>();
      for (const w of wsWindows) {
        const col = w.layout?.pos_in_scrolling_layout?.[0] ?? 1;
        if (!columns.has(col)) columns.set(col, []);
        columns.get(col)!.push(w);
      }

      const sortedCols = [...columns.keys()].sort((a, b) => a - b);
      const rendered: RenderedWindow[] = [];
      let xCursor = 0;
      let maxHeight = 0;

      for (const col of sortedCols) {
        const colWindows = columns.get(col)!.sort(
          (a, b) => (a.layout?.pos_in_scrolling_layout?.[1] ?? 0) - (b.layout?.pos_in_scrolling_layout?.[1] ?? 0)
        );
        const colWidth = Math.max(...colWindows.map(w => w.layout?.tile_size?.[0] ?? 800));
        let yCursor = 0;
        for (const win of colWindows) {
          const h = win.layout?.tile_size?.[1] ?? monitorHeight;
          const w = win.layout?.tile_size?.[0] ?? colWidth;
          rendered.push({ win, x: xCursor, y: yCursor, w, h });
          yCursor += h;

          if (ws.is_active && win.id === ws.active_window_id) {
            const centerX = xCursor + w / 2;
            initialPanX = -(Math.max(0, centerX - monitorWidth / 2));
          }
        }
        maxHeight = Math.max(maxHeight, yCursor);
        xCursor += colWidth;
      }

      maxTotalWidth = Math.max(maxTotalWidth, xCursor);
      layouts.push({ ws, totalWidth: xCursor, totalHeight: maxHeight, windows: rendered });
    }

    return { layouts, maxTotalWidth, initialPanX };
  }

  let layoutData = $derived(buildLayouts());
  let maxPan = $derived(Math.max(0, layoutData.maxTotalWidth - monitorWidth));

  // Clamp panX within bounds
  function clampPan(x: number): number {
    return Math.max(-maxPan * scale, Math.min(0, x));
  }

  // Set initial pan on first data load
  let hasInitialized = false;
  $effect(() => {
    if (layoutData.layouts.length > 0 && !hasInitialized) {
      panX = clampPan(layoutData.initialPanX * scale);
      hasInitialized = true;
    }
  });

  // Touch handlers for horizontal panning
  function handleTouchStart(e: TouchEvent) {
    if (momentumRaf) { cancelAnimationFrame(momentumRaf); momentumRaf = null; }
    isPanning = true;
    panStartX = e.touches[0].clientX;
    panStartOffset = panX;
    panVelocity = 0;
    lastPanX = e.touches[0].clientX;
    lastPanTime = performance.now();
  }

  function handleTouchMove(e: TouchEvent) {
    if (!isPanning) return;
    e.preventDefault();
    const x = e.touches[0].clientX;
    const now = performance.now();
    const dt = now - lastPanTime;
    if (dt > 0) panVelocity = (x - lastPanX) / dt;
    lastPanX = x;
    lastPanTime = now;
    panX = clampPan(panStartOffset + (x - panStartX));
  }

  function handleTouchEnd() {
    isPanning = false;
    // Momentum scrolling
    const decel = 0.95;
    function momentum() {
      panVelocity *= decel;
      if (Math.abs(panVelocity) < 0.01) return;
      panX = clampPan(panX + panVelocity * 16);
      momentumRaf = requestAnimationFrame(momentum);
    }
    momentumRaf = requestAnimationFrame(momentum);
  }

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
    } catch {}
  }

  function focusWindow(id: number) {
    api.post('/niri/action', { action: 'focus-window', args: { id } }).catch(() => {});
  }

  onMount(() => {
    fetchAll();
    cleanups.push(
      api.on('niri_state', (msg: any) => {
        if (Array.isArray(msg.windows)) windows = msg.windows;
        if (Array.isArray(msg.workspaces)) workspaces = msg.workspaces;
        if (msg.outputs && typeof msg.outputs === 'object' && !Array.isArray(msg.outputs)) outputs = msg.outputs;
      })
    );
    if (containerEl) {
      containerWidth = containerEl.clientWidth;
      const ro = new ResizeObserver(([entry]) => { containerWidth = entry.contentRect.width; });
      ro.observe(containerEl);
      cleanups.push(() => ro.disconnect());
    }
  });

  onDestroy(() => {
    if (momentumRaf) cancelAnimationFrame(momentumRaf);
    cleanups.forEach(fn => fn());
  });
</script>

<div class="windows-page" bind:this={containerEl}>
  {#if multiMonitor}
    <div class="output-selector">
      {#each activeOutputs as output}
        <button
          class="output-tab"
          class:active={selectedOutput === output.name}
          onclick={() => { selectedOutput = output.name; hasInitialized = false; }}
        >
          {(output.model || output.name).toUpperCase()}
        </button>
      {/each}
    </div>
  {/if}

  <div
    class="workspace-list"
    ontouchstart={handleTouchStart}
    ontouchmove={handleTouchMove}
    ontouchend={handleTouchEnd}
  >
    {#each layoutData.layouts as layout, wsIdx}
      {@const isActive = layout.ws.is_active}
      <div class="workspace-row" class:active={isActive}>
        <div class="workspace-header">
          <span class="ws-idx">{layout.ws.name?.toUpperCase() || wsIdx + 1}</span>
          {#if isActive}<span class="active-marker"></span>{/if}
        </div>
        <div
          class="workspace-canvas"
          style="height: {layout.totalHeight * scale}px"
        >
          {#each layout.windows as rw}
            <button
              class="win-tile"
              class:focused={rw.win.is_focused}
              class:active-win={rw.win.id === layout.ws.active_window_id}
              style="
                left: {rw.x * scale + panX}px;
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
    {/each}

    {#if layoutData.layouts.length === 0}
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

  .workspace-list {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    touch-action: pan-y;
    padding: 8px 0 24px;
  }

  .workspace-row {
    margin-bottom: 8px;
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

  .workspace-row.active .ws-idx {
    color: #888888;
  }

  .active-marker {
    width: 4px;
    height: 4px;
    background: #ff2d2d;
  }

  .workspace-canvas {
    position: relative;
    overflow: hidden;
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
