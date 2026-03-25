<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '$lib/api.svelte';

  // Touch tracking state
  let touchArea: HTMLDivElement;
  let activeTouches = new Map<number, { x: number; y: number; startX: number; startY: number; startTime: number }>();
  let pendingDx = 0;
  let pendingDy = 0;
  let pendingScrollX = 0;
  let pendingScrollY = 0;
  let rafId: number | null = null;
  let lastTouchCount = 0;

  // Keyboard state
  let keyboardOpen = $state(false);
  let hiddenInput: HTMLInputElement;
  let modifiers = $state({
    ctrl: false,
    alt: false,
    super_: false,
    shift: false,
  });

  // Batched send loop
  function sendBatched() {
    if (pendingDx !== 0 || pendingDy !== 0) {
      api.send({ type: 'mouse_move', dx: pendingDx, dy: pendingDy });
      pendingDx = 0;
      pendingDy = 0;
    }
    if (pendingScrollX !== 0 || pendingScrollY !== 0) {
      api.send({ type: 'scroll', dx: pendingScrollX, dy: pendingScrollY });
      pendingScrollX = 0;
      pendingScrollY = 0;
    }
    rafId = requestAnimationFrame(sendBatched);
  }

  function handleTouchStart(e: TouchEvent) {
    e.preventDefault();
    for (let i = 0; i < e.changedTouches.length; i++) {
      const t = e.changedTouches[i];
      activeTouches.set(t.identifier, {
        x: t.clientX,
        y: t.clientY,
        startX: t.clientX,
        startY: t.clientY,
        startTime: Date.now(),
      });
    }
    lastTouchCount = e.touches.length;
  }

  function handleTouchMove(e: TouchEvent) {
    e.preventDefault();
    const touchCount = e.touches.length;

    for (let i = 0; i < e.changedTouches.length; i++) {
      const t = e.changedTouches[i];
      const prev = activeTouches.get(t.identifier);
      if (!prev) continue;

      const dx = t.clientX - prev.x;
      const dy = t.clientY - prev.y;

      if (touchCount === 1) {
        // Single finger drag -> mouse move
        pendingDx += dx;
        pendingDy += dy;
      } else if (touchCount === 2) {
        // Two finger drag -> scroll
        pendingScrollX += dx;
        pendingScrollY += dy;
      }

      activeTouches.set(t.identifier, {
        ...prev,
        x: t.clientX,
        y: t.clientY,
      });
    }
  }

  function handleTouchEnd(e: TouchEvent) {
    e.preventDefault();
    const now = Date.now();

    for (let i = 0; i < e.changedTouches.length; i++) {
      const t = e.changedTouches[i];
      const prev = activeTouches.get(t.identifier);
      if (!prev) continue;

      const elapsed = now - prev.startTime;
      const dx = Math.abs(t.clientX - prev.startX);
      const dy = Math.abs(t.clientY - prev.startY);
      const moved = Math.sqrt(dx * dx + dy * dy);

      // Check for tap (short duration, minimal movement)
      if (elapsed < 200 && moved < 10) {
        if (lastTouchCount === 1 && e.touches.length === 0) {
          // Single tap -> left click
          api.send({ type: 'mouse_button', button: 'left', state: 'press' });
          api.send({ type: 'mouse_button', button: 'left', state: 'release' });
        } else if (lastTouchCount === 2 && e.touches.length <= 1) {
          // Two-finger tap -> right click
          api.send({ type: 'mouse_button', button: 'right', state: 'press' });
          api.send({ type: 'mouse_button', button: 'right', state: 'release' });
        }
      }

      activeTouches.delete(t.identifier);
    }

    if (e.touches.length === 0) {
      lastTouchCount = 0;
    }
  }

  function handleTouchCancel(e: TouchEvent) {
    e.preventDefault();
    for (let i = 0; i < e.changedTouches.length; i++) {
      activeTouches.delete(e.changedTouches[i].identifier);
    }
    if (e.touches.length === 0) {
      lastTouchCount = 0;
    }
  }

  function toggleKeyboard() {
    keyboardOpen = !keyboardOpen;
    if (keyboardOpen) {
      // Focus hidden input on next tick to bring up mobile keyboard
      requestAnimationFrame(() => {
        hiddenInput?.focus();
      });
    } else {
      hiddenInput?.blur();
    }
  }

  function toggleModifier(mod: 'ctrl' | 'alt' | 'super_' | 'shift') {
    modifiers[mod] = !modifiers[mod];
  }

  function getActiveModifiers(): string[] {
    const mods: string[] = [];
    if (modifiers.ctrl) mods.push('ctrl');
    if (modifiers.alt) mods.push('alt');
    if (modifiers.super_) mods.push('super');
    if (modifiers.shift) mods.push('shift');
    return mods;
  }

  function handleKeyDown(e: KeyboardEvent) {
    e.preventDefault();
    const key = mapKey(e.key);
    if (key) {
      api.send({
        type: 'key_press',
        key,
        modifiers: getActiveModifiers(),
      });
    }
  }

  function handleKeyUp(e: KeyboardEvent) {
    e.preventDefault();
    const key = mapKey(e.key);
    if (key) {
      api.send({
        type: 'key_release',
        key,
        modifiers: getActiveModifiers(),
      });
    }
  }

  function mapKey(key: string): string | null {
    // Map browser key names to our protocol key names
    const keyMap: Record<string, string> = {
      'Enter': 'Return',
      'Backspace': 'BackSpace',
      'Tab': 'Tab',
      'Escape': 'Escape',
      ' ': 'space',
      'ArrowUp': 'Up',
      'ArrowDown': 'Down',
      'ArrowLeft': 'Left',
      'ArrowRight': 'Right',
      'Delete': 'Delete',
      'Home': 'Home',
      'End': 'End',
      'PageUp': 'Page_Up',
      'PageDown': 'Page_Down',
      'Control': null,
      'Alt': null,
      'Shift': null,
      'Meta': null,
    };

    if (key in keyMap) return keyMap[key];
    if (key.length === 1) return key;
    return key;
  }

  // Handle input events for mobile keyboards that don't fire proper key events
  function handleInput(e: Event) {
    const input = e.target as HTMLInputElement;
    const value = input.value;
    if (value) {
      // Send each character as a key press+release
      for (const char of value) {
        api.send({
          type: 'key_press',
          key: char,
          modifiers: getActiveModifiers(),
        });
        api.send({
          type: 'key_release',
          key: char,
          modifiers: getActiveModifiers(),
        });
      }
      input.value = '';
    }
  }

  onMount(() => {
    rafId = requestAnimationFrame(sendBatched);
  });

  onDestroy(() => {
    if (rafId !== null) cancelAnimationFrame(rafId);
  });
</script>

<div class="trackpad-container">
  <div
    class="touch-area"
    bind:this={touchArea}
    ontouchstart={handleTouchStart}
    ontouchmove={handleTouchMove}
    ontouchend={handleTouchEnd}
    ontouchcancel={handleTouchCancel}
    role="application"
    aria-label="Trackpad touch area"
  >
    <div class="touch-hint">
      <p>Touch to control mouse</p>
      <p class="hint-detail">1 finger: move | tap: click</p>
      <p class="hint-detail">2 fingers: scroll | 2-tap: right click</p>
    </div>
  </div>

  {#if keyboardOpen}
    <div class="modifier-row">
      <button
        class="mod-key"
        class:active={modifiers.ctrl}
        onclick={() => toggleModifier('ctrl')}
      >Ctrl</button>
      <button
        class="mod-key"
        class:active={modifiers.alt}
        onclick={() => toggleModifier('alt')}
      >Alt</button>
      <button
        class="mod-key"
        class:active={modifiers.super_}
        onclick={() => toggleModifier('super_')}
      >Super</button>
      <button
        class="mod-key"
        class:active={modifiers.shift}
        onclick={() => toggleModifier('shift')}
      >Shift</button>
    </div>
  {/if}

  <input
    bind:this={hiddenInput}
    class="hidden-input"
    type="text"
    autocomplete="off"
    autocapitalize="off"
    autocorrect="off"
    spellcheck="false"
    onkeydown={handleKeyDown}
    onkeyup={handleKeyUp}
    oninput={handleInput}
  />

  <button class="keyboard-toggle" onclick={toggleKeyboard} class:active={keyboardOpen} aria-label="Toggle keyboard">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <rect x="2" y="4" width="20" height="14" rx="2" />
      <line x1="6" y1="8" x2="6" y2="8" />
      <line x1="10" y1="8" x2="10" y2="8" />
      <line x1="14" y1="8" x2="14" y2="8" />
      <line x1="18" y1="8" x2="18" y2="8" />
      <line x1="6" y1="12" x2="6" y2="12" />
      <line x1="18" y1="12" x2="18" y2="12" />
      <line x1="8" y1="16" x2="16" y2="16" />
    </svg>
  </button>
</div>

<style>
  .trackpad-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    position: relative;
    overflow: hidden;
  }

  .touch-area {
    flex: 1;
    background: #16213e;
    border: 1px solid #0f3460;
    margin: 8px;
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
  }

  .touch-hint {
    text-align: center;
    color: #333;
    pointer-events: none;
  }

  .touch-hint p {
    font-size: 16px;
    font-weight: 500;
    margin-bottom: 8px;
    color: #444;
  }

  .touch-hint .hint-detail {
    font-size: 12px;
    color: #333;
    margin-bottom: 4px;
  }

  .keyboard-toggle {
    position: absolute;
    bottom: 12px;
    right: 12px;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: #16213e;
    border: 1px solid #0f3460;
    color: #666;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.2s;
    z-index: 10;
  }

  .keyboard-toggle:active,
  .keyboard-toggle.active {
    background: #0f3460;
    color: #e94560;
    border-color: #e94560;
  }

  .keyboard-toggle svg {
    width: 24px;
    height: 24px;
  }

  .modifier-row {
    display: flex;
    gap: 6px;
    padding: 6px 8px;
    background: #16213e;
    border-top: 1px solid #0f3460;
    flex-shrink: 0;
  }

  .mod-key {
    flex: 1;
    padding: 10px 0;
    background: #1a1a2e;
    border: 1px solid #0f3460;
    border-radius: 8px;
    color: #888;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    text-align: center;
  }

  .mod-key.active {
    background: #0f3460;
    color: #e94560;
    border-color: #e94560;
  }

  .hidden-input {
    position: absolute;
    bottom: 0;
    left: 50%;
    width: 1px;
    height: 1px;
    opacity: 0;
    pointer-events: none;
  }
</style>
