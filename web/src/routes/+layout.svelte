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
    { path: '/', label: 'TRACKPAD' },
    { path: '/media', label: 'MEDIA' },
    { path: '/windows', label: 'WINDOWS' },
    { path: '/settings', label: 'SETTINGS' },
  ] as const;

  let currentPath = $derived($page.url.pathname);

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
        {tab.label}
      </a>
    {/each}
  </nav>
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
    background: #111111;
    color: #ffffff;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Helvetica Neue', Helvetica, Arial, sans-serif;
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
    border-top: 1px solid #222222;
    padding-bottom: env(safe-area-inset-bottom, 0);
    flex-shrink: 0;
    background: #111111;
  }

  .tab {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 14px 0;
    text-decoration: none;
    color: #555555;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.1em;
    transition: color 0.15s;
  }

  .tab.active {
    color: #ffffff;
  }
</style>
