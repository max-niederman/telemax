<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api.svelte';
  import { page } from '$app/stores';
  import { base } from '$app/paths';

  let { children } = $props();

  onMount(() => {
    api.connectWs();
  });

  const tabs = [
    { path: '/', label: 'Trackpad', icon: 'trackpad' },
    { path: '/media', label: 'Media', icon: 'media' },
    { path: '/windows', label: 'Windows', icon: 'windows' },
    { path: '/settings', label: 'Settings', icon: 'settings' },
  ] as const;

  let currentPath = $derived($page.url.pathname);
  let connected = $derived(api.connected);

  function isActive(tabPath: string): boolean {
    const full = base + tabPath;
    if (tabPath === '/') return currentPath === full || currentPath === base || currentPath === base + '/';
    return currentPath.startsWith(full);
  }
</script>

<div class="app">
  <main class="content">
    {@render children()}
  </main>

  <nav class="tab-bar">
    {#each tabs as tab}
      <a
        href="{base}{tab.path}"
        class="tab"
        class:active={isActive(tab.path)}
      >
        <svg class="tab-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          {#if tab.icon === 'trackpad'}
            <rect x="2" y="4" width="20" height="16" rx="3" />
            <line x1="2" y1="16" x2="22" y2="16" />
          {:else if tab.icon === 'media'}
            <circle cx="12" cy="12" r="10" />
            <polygon points="10,8 16,12 10,16" fill="currentColor" stroke="none" />
          {:else if tab.icon === 'windows'}
            <rect x="3" y="3" width="7" height="7" rx="1" />
            <rect x="14" y="3" width="7" height="7" rx="1" />
            <rect x="3" y="14" width="7" height="7" rx="1" />
            <rect x="14" y="14" width="7" height="7" rx="1" />
          {:else if tab.icon === 'settings'}
            <circle cx="12" cy="12" r="3" />
            <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
          {/if}
        </svg>
        <span class="tab-label">{tab.label}</span>
      </a>
    {/each}
  </nav>

  {#if !connected}
    <div class="connection-status">Disconnected</div>
  {/if}
</div>

<style>
  :global(*) {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
    -webkit-tap-highlight-color: transparent;
  }

  :global(html, body) {
    height: 100%;
    overflow: hidden;
    background: #1a1a2e;
    color: #e0e0e0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    touch-action: none;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100dvh;
    position: relative;
  }

  .content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .tab-bar {
    display: flex;
    background: #16213e;
    border-top: 1px solid #0f3460;
    padding-bottom: env(safe-area-inset-bottom, 0);
    flex-shrink: 0;
  }

  .tab {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 8px 0;
    text-decoration: none;
    color: #666;
    transition: color 0.2s;
  }

  .tab.active {
    color: #e94560;
  }

  .tab-icon {
    width: 24px;
    height: 24px;
    margin-bottom: 2px;
  }

  .tab-label {
    font-size: 10px;
    font-weight: 500;
  }

  .connection-status {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    background: #e94560;
    color: white;
    text-align: center;
    padding: 4px;
    font-size: 12px;
    font-weight: 600;
    z-index: 100;
  }
</style>
