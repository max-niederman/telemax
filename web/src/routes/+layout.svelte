<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api.svelte';
  import { page } from '$app/stores';
  import { base } from '$app/paths';

  let { children } = $props();

  let digits = $state<string[]>(['', '', '', '', '', '']);
  let pairError = $state('');
  let pairing = $state(false);
  let digitRefs: HTMLInputElement[] = [];

  onMount(async () => {
    await api.checkAuth();
    if (!api.needsPairing) {
      api.connectWs();
    }
  });

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

  function handleDigitInput(index: number, e: Event) {
    const input = e.target as HTMLInputElement;
    const val = input.value.replace(/\D/g, '');
    digits[index] = val.slice(-1);
    input.value = digits[index];

    if (digits[index] && index < 5) {
      digitRefs[index + 1]?.focus();
    }

    // Check if all 6 digits are filled
    if (digits.every(d => d.length === 1)) {
      submitCode();
    }
  }

  function handleDigitKeydown(index: number, e: KeyboardEvent) {
    if (e.key === 'Backspace' && !digits[index] && index > 0) {
      digitRefs[index - 1]?.focus();
    }
  }

  function handleDigitPaste(e: ClipboardEvent) {
    e.preventDefault();
    const text = e.clipboardData?.getData('text')?.replace(/\D/g, '') ?? '';
    if (text.length === 6) {
      for (let i = 0; i < 6; i++) {
        digits[i] = text[i];
        if (digitRefs[i]) digitRefs[i].value = text[i];
      }
      submitCode();
    }
  }

  async function submitCode() {
    const code = digits.join('');
    if (code.length !== 6) return;
    pairing = true;
    pairError = '';
    const success = await api.pair(code);
    pairing = false;
    if (success) {
      api.connectWs();
    } else {
      pairError = 'INVALID CODE';
      digits = ['', '', '', '', '', ''];
      digitRefs[0]?.focus();
    }
  }
</script>

{#if api.needsPairing}
  <div class="pair-screen">
    <div class="pair-container">
      <h1 class="pair-title">TELEMAX</h1>
      <p class="pair-subtitle">ENTER PAIRING CODE</p>

      <div class="digit-row">
        {#each digits as digit, i}
          <input
            class="digit-box"
            type="text"
            inputmode="numeric"
            maxlength="1"
            value={digit}
            bind:this={digitRefs[i]}
            oninput={(e: Event) => handleDigitInput(i, e)}
            onkeydown={(e: KeyboardEvent) => handleDigitKeydown(i, e)}
            onpaste={handleDigitPaste}
            disabled={pairing}
            autocomplete="off"
          />
        {/each}
      </div>

      {#if pairError}
        <p class="pair-error">{pairError}</p>
      {/if}

      <p class="pair-hint">Check your desktop for the pairing code</p>
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
    gap: 24px;
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

  .digit-row {
    display: flex;
    gap: 8px;
  }

  .digit-box {
    width: 48px;
    height: 64px;
    border: 1px solid #333333;
    background: transparent;
    color: #ffffff;
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 28px;
    font-weight: 500;
    text-align: center;
    outline: none;
    caret-color: #ff2d2d;
    transition: border-color 0.15s;
  }

  .digit-box:focus {
    border-color: #ff2d2d;
  }

  .digit-box:disabled {
    opacity: 0.4;
  }

  .pair-error {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.15em;
    color: #ff2d2d;
  }

  .pair-hint {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 300;
    letter-spacing: 0.1em;
    color: #333333;
  }
</style>
