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
  let scrollArea: HTMLDivElement;
  let hasScrolledInitially = false;
  let cleanups: (() => void)[] = [];

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

  let monitorWidth = $derived(
    outputs[selectedOutput ?? '']?.logical?.width ?? 3840
  );
  let monitorHeight = $derived(
    outputs[selectedOutput ?? '']?.logical?.height ?? 2160
  );

  let scale = $derived(containerWidth / monitorWidth);

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
  }

  // Find the widest workspace to set the shared canvas width
  function buildLayouts(): { layouts: RenderedWorkspace[]; maxTotalWidth: number; initialScrollX: number } {
    const layouts: RenderedWorkspace[] = [];
    let maxTotalWidth = monitorWidth; // at minimum, the monitor width
    let initialScrollX = 0;

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

          // Find scroll position: center the active window of the active workspace
          if (ws.is_active && win.id === ws.active_window_id) {
            const centerX = xCursor + w / 2;
            initialScrollX = Math.max(0, centerX - monitorWidth / 2);
          }
        }
        maxHeight = Math.max(maxHeight, yCursor);
        xCursor += colWidth;
      }

      const totalWidth = xCursor;
      maxTotalWidth = Math.max(maxTotalWidth, totalWidth);
      layouts.push({ ws, totalWidth, totalHeight: maxHeight, windows: rendered });
    }

    return { layouts, maxTotalWidth, initialScrollX };
  }

  let layoutData = $derived(buildLayouts());

  // Set initial scroll to center the active window
  $effect(() => {
    if (scrollArea && layoutData.layouts.length > 0 && !hasScrolledInitially) {
      scrollArea.scrollLeft = layoutData.initialScrollX * scale;
      hasScrolledInitially = true;
    }
  });

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

<div class="windows-page" bind:this={containerEl}>
  {#if multiMonitor}
    <div class="output-selector">
      {#each activeOutputs as output}
        <button
          class="output-tab"
          class:active={selectedOutput === output.name}
          onclick={() => { selectedOutput = output.name; hasScrolledInitially = false; }}
        >
          {(output.model || output.name).toUpperCase()}
        </button>
      {/each}
    </div>
  {/if}

  <div class="scroll-area" bind:this={scrollArea}>
    <div
      class="scroll-canvas"
      style="width: {layoutData.maxTotalWidth * scale}px"
    >
      {#each layoutData.layouts as layout, wsIdx}
        {@const viewportHeight = monitorHeight * scale}
        {@const isActive = layout.ws.is_active}
        <div class="workspace-row" class:active={isActive}>
          <div class="workspace-header">
            <span class="ws-idx">{layout.ws.name?.toUpperCase() || wsIdx + 1}</span>
            {#if isActive}<span class="active-marker"></span>{/if}
          </div>
          <div class="workspace-canvas" style="height: {layout.totalHeight * scale}px">
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
      {/each}
    </div>
  </div>

  {#if layoutData.layouts.length === 0}
    <div class="empty">NO WINDOWS</div>
  {/if}
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

  /* Single horizontal+vertical scroll area for all workspaces */
  .scroll-area {
    flex: 1;
    overflow: auto;
    touch-action: pan-x pan-y;
    scrollbar-width: none;
  }
  .scroll-area::-webkit-scrollbar { display: none; }

  .scroll-canvas {
    min-width: 100%;
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
    position: sticky;
    left: 0;
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
    min-width: 100%;
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
