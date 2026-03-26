<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api.svelte';
  import { page } from '$app/stores';
  import { base } from '$app/paths';

  let { children } = $props();

  let pairingCode = $state('');
  let pairStatus = $state<'idle' | 'registering' | 'waiting' | 'error' | 'expired'>('idle');
  let pollInterval: ReturnType<typeof setInterval> | undefined;

  onMount(async () => {
    await api.checkAuth();
    if (!api.needsPairing) {
      api.connectWs();
    } else {
      startPairing();
    }
  });

  async function startPairing() {
    pairingCode = api.generateCode();
    pairStatus = 'registering';

    const ok = await api.requestPairing(pairingCode);
    if (!ok) {
      // Code collision, try again
      pairingCode = api.generateCode();
      const retry = await api.requestPairing(pairingCode);
      if (!retry) {
        pairStatus = 'error';
        return;
      }
    }
    pairStatus = 'waiting';

    // Poll every 2 seconds
    pollInterval = setInterval(async () => {
      try {
        const token = await api.pollPairing(pairingCode);
        if (token) {
          clearInterval(pollInterval);
          pairStatus = 'idle';
          api.connectWs();
        }
      } catch {
        clearInterval(pollInterval);
        pairStatus = 'expired';
      }
    }, 2000);
  }

  async function retryPairing() {
    clearInterval(pollInterval);
    startPairing();
  }

  const tabs = [
    { path: '/trackpad', label: 'TRACKPAD' },
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

{#if api.needsPairing}
  <div class="pair-screen">
    <div class="pair-container">
      <h1 class="pair-title">TELEMAX</h1>

      {#if pairStatus === 'waiting' || pairStatus === 'registering'}
        <p class="pair-subtitle">APPROVE ON DESKTOP</p>

        <div class="code-display">
          {#each pairingCode.split('') as digit}
            <span class="code-digit">{digit}</span>
          {/each}
        </div>

        <div class="pair-instructions">
          <p>Run on your desktop:</p>
          <code class="pair-command">echo {pairingCode} | nc -U $XDG_RUNTIME_DIR/telemax-code.sock</code>
        </div>

        {#if pairStatus === 'waiting'}
          <p class="pair-hint">WAITING FOR APPROVAL</p>
        {:else}
          <p class="pair-hint">CONNECTING</p>
        {/if}

      {:else if pairStatus === 'expired'}
        <p class="pair-subtitle">CODE EXPIRED</p>
        <button class="retry-btn" onclick={retryPairing}>RETRY</button>

      {:else if pairStatus === 'error'}
        <p class="pair-subtitle">PAIRING FAILED</p>
        <button class="retry-btn" onclick={retryPairing}>RETRY</button>
      {/if}
    </div>
  </div>
{:else}
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
{/if}

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

  /* Pairing screen */
  .pair-screen {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100dvh;
    padding: 24px;
  }

  .pair-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 32px;
    max-width: 400px;
    width: 100%;
  }

  .pair-title {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 32px;
    font-weight: 700;
    letter-spacing: 0.3em;
    color: #ffffff;
  }

  .pair-subtitle {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.2em;
    color: #666666;
  }

  .code-display {
    display: flex;
    gap: 12px;
  }

  .code-digit {
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 40px;
    font-weight: 500;
    color: #ffffff;
    width: 48px;
    height: 64px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid #333333;
  }

  .pair-instructions {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .pair-instructions p {
    font-size: 11px;
    font-weight: 300;
    letter-spacing: 0.1em;
    color: #666666;
  }

  .pair-command {
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 11px;
    color: #999999;
    background: #1a1a1a;
    padding: 8px 12px;
    border: 1px solid #333333;
    user-select: all;
    -webkit-user-select: all;
    word-break: break-all;
  }

  .pair-hint {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.15em;
    color: #333333;
  }

  .retry-btn {
    background: transparent;
    border: 1px solid #333333;
    color: #ffffff;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.15em;
    padding: 14px 32px;
    cursor: pointer;
  }

  .retry-btn:active {
    color: #ff2d2d;
    border-color: #ff2d2d;
  }
</style>
