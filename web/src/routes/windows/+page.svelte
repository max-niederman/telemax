<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '$lib/api.svelte';

  interface NiriWindow {
    id: number;
    title: string;
    app_id?: string;
    workspace_id?: number;
    is_focused?: boolean;
    layout?: {
      pos_in_scrolling_layout?: [number, number];
      tile_size?: [number, number];
      window_size?: [number, number];
    };
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
    name: string;
    make?: string;
    model?: string;
    logical?: {
      width: number;
      height: number;
      scale: number;
    };
  }

  let windows = $state<NiriWindow[]>([]);
  let workspaces = $state<NiriWorkspace[]>([]);
  let outputs = $state<Record<string, NiriOutput>>({});
  let selectedOutput = $state<string | null>(null);
  let cleanups: (() => void)[] = [];

  // Active outputs (ones with a logical config = currently connected)
  let activeOutputs = $derived(
    Object.entries(outputs)
      .filter(([_, o]) => o.logical)
      .map(([name, o]) => ({ name, ...o }))
  );

  let multiMonitor = $derived(activeOutputs.length > 1);

  // Auto-select the focused output
  $effect(() => {
    if (!selectedOutput && activeOutputs.length > 0) {
      const focusedWs = workspaces.find(ws => ws.is_focused);
      selectedOutput = focusedWs?.output ?? activeOutputs[0].name;
    }
  });

  // Workspaces for the selected output, sorted by index
  let outputWorkspaces = $derived(
    workspaces
      .filter(ws => ws.output === selectedOutput)
      .sort((a, b) => a.idx - b.idx)
  );

  // Build spatial layout for a workspace
  function getWorkspaceLayout(wsId: number) {
    const wsWindows = windows.filter(w => w.workspace_id === wsId);
    if (wsWindows.length === 0) return null;

    // Find bounds
    let minCol = Infinity, maxCol = -Infinity;
    let totalWidth = 0;
    let maxHeight = 0;

    // Group by column
    const columns = new Map<number, NiriWindow[]>();
    for (const w of wsWindows) {
      const col = w.layout?.pos_in_scrolling_layout?.[0] ?? 1;
      if (!columns.has(col)) columns.set(col, []);
      columns.get(col)!.push(w);
      minCol = Math.min(minCol, col);
      maxCol = Math.max(maxCol, col);
    }

    // Compute total width and max height
    const sortedCols = [...columns.keys()].sort((a, b) => a - b);
    const colWidths: number[] = [];
    for (const col of sortedCols) {
      const colWindows = columns.get(col)!;
      const width = Math.max(...colWindows.map(w => w.layout?.tile_size?.[0] ?? 800));
      colWidths.push(width);
      totalWidth += width;

      let colHeight = 0;
      for (const w of colWindows) {
        colHeight += w.layout?.tile_size?.[1] ?? 1080;
      }
      maxHeight = Math.max(maxHeight, colHeight);
    }

    return { columns, sortedCols, colWidths, totalWidth, maxHeight };
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
    } catch {
      // ignore
    }
  }

  async function focusWindow(id: number) {
    try {
      await api.post('/niri/action', { action: 'focus-window', args: { id } });
      // Re-fetch to update focus state
      setTimeout(fetchAll, 200);
    } catch {
      // ignore
    }
  }

  onMount(() => {
    fetchAll();
    cleanups.push(
      api.on('niri_event', () => { fetchAll(); })
    );
  });

  onDestroy(() => {
    cleanups.forEach(fn => fn());
  });
</script>

<div class="windows-page">
  <!-- Monitor selector (hidden if single monitor) -->
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

  <!-- Workspace rows -->
  <div class="workspace-scroll">
    {#each outputWorkspaces as ws, wsIdx}
      {@const layout = getWorkspaceLayout(ws.id)}
      <div class="workspace-row" class:active={ws.is_active}>
        <div class="workspace-label">
          {ws.name?.toUpperCase() || wsIdx + 1}
          {#if ws.is_active}
            <span class="active-dot"></span>
          {/if}
        </div>

        {#if layout}
          <div class="columns-scroll">
            <div
              class="columns-container"
              style="--total-cols: {layout.sortedCols.length}"
            >
              {#each layout.sortedCols as col, colIdx}
                {@const colWindows = layout.columns.get(col) ?? []}
                <div
                  class="column"
                  style="flex: {layout.colWidths[colIdx]}"
                >
                  {#each colWindows.sort((a, b) => (a.layout?.pos_in_scrolling_layout?.[1] ?? 0) - (b.layout?.pos_in_scrolling_layout?.[1] ?? 0)) as win}
                    <button
                      class="window-tile"
                      class:focused={win.is_focused}
                      onclick={() => focusWindow(win.id)}
                      style="flex: {win.layout?.tile_size?.[1] ?? 1080}"
                    >
                      <span class="window-app">{(win.app_id || '?').toUpperCase()}</span>
                      <span class="window-title">{win.title}</span>
                    </button>
                  {/each}
                </div>
              {/each}
            </div>
          </div>
        {:else}
          <div class="empty-workspace">EMPTY</div>
        {/if}
      </div>
    {/each}

    {#if outputWorkspaces.length === 0}
      <div class="empty-state">NO WORKSPACES</div>
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

  /* Monitor selector */
  .output-selector {
    display: flex;
    gap: 0;
    flex-shrink: 0;
    border-bottom: 1px solid #333333;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .output-selector::-webkit-scrollbar {
    display: none;
  }

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

  /* Workspace scroll area */
  .workspace-scroll {
    flex: 1;
    overflow-y: auto;
    touch-action: pan-y;
    padding: 8px 0;
  }

  .workspace-row {
    margin-bottom: 2px;
    border-bottom: 1px solid #1a1a1a;
  }

  .workspace-label {
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 11px;
    font-weight: 700;
    color: #333333;
    letter-spacing: 0.1em;
    padding: 8px 12px 4px;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .active-dot {
    width: 5px;
    height: 5px;
    background: #ff2d2d;
    display: inline-block;
  }

  /* Horizontal scroll for columns within a workspace */
  .columns-scroll {
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
    padding: 0 8px 8px;
  }

  .columns-scroll::-webkit-scrollbar {
    display: none;
  }

  .columns-container {
    display: flex;
    gap: 2px;
    min-width: min-content;
    height: 120px;
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 100px;
    max-width: 280px;
  }

  .window-tile {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 6px 10px;
    background: #1a1a1a;
    border: 1px solid #2a2a2a;
    color: inherit;
    cursor: pointer;
    text-align: left;
    overflow: hidden;
    min-height: 0;
  }

  .window-tile.focused {
    border-color: #ff2d2d;
  }

  .window-tile:active {
    background: #222222;
  }

  .window-app {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 9px;
    font-weight: 700;
    color: #666666;
    letter-spacing: 0.1em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1.2;
  }

  .window-title {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 300;
    color: #cccccc;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1.3;
  }

  .window-tile.focused .window-title {
    color: #ffffff;
  }

  .empty-workspace {
    padding: 12px;
    text-align: center;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.15em;
    color: #222222;
  }

  .empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.2em;
    color: #333333;
  }
</style>
