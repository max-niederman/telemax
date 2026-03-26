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
      // Must be synchronous within the click handler for iOS to open keyboard
      hiddenInput?.focus();
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
      <p>TOUCH TO CONTROL</p>
      <p class="hint-detail">1 FINGER: MOVE — TAP: CLICK</p>
      <p class="hint-detail">2 FINGERS: SCROLL — 2-TAP: RIGHT CLICK</p>
    </div>
  </div>

  {#if keyboardOpen}
    <div class="modifier-row">
      <button
        class="mod-key"
        class:active={modifiers.ctrl}
        onclick={() => toggleModifier('ctrl')}
      >CTRL</button>
      <button
        class="mod-key"
        class:active={modifiers.alt}
        onclick={() => toggleModifier('alt')}
      >ALT</button>
      <button
        class="mod-key"
        class:active={modifiers.super_}
        onclick={() => toggleModifier('super_')}
      >SUPER</button>
      <button
        class="mod-key"
        class:active={modifiers.shift}
        onclick={() => toggleModifier('shift')}
      >SHIFT</button>
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
    KB
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
    background: #141414;
    border: 1px solid #333333;
    border-bottom: none;
    display: flex;
    align-items: center;
    justify-content: center;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
  }

  .touch-hint {
    text-align: center;
    pointer-events: none;
  }

  .touch-hint p {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.15em;
    color: #333333;
    margin-bottom: 8px;
  }

  .touch-hint .hint-detail {
    font-size: 10px;
    font-weight: 300;
    letter-spacing: 0.12em;
    color: #333333;
    margin-bottom: 4px;
  }

  .keyboard-toggle {
    position: absolute;
    bottom: 12px;
    right: 12px;
    background: transparent;
    border: none;
    color: #666666;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    cursor: pointer;
    padding: 12px;
    z-index: 10;
  }

  .keyboard-toggle.active {
    color: #ff2d2d;
  }

  .modifier-row {
    display: flex;
    gap: 0;
    border-top: 1px solid #333333;
    flex-shrink: 0;
  }

  .mod-key {
    flex: 1;
    padding: 14px 0;
    background: transparent;
    border: none;
    border-right: 1px solid #333333;
    color: #666666;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    cursor: pointer;
    text-align: center;
  }

  .mod-key:last-child {
    border-right: none;
  }

  .mod-key.active {
    color: #ff2d2d;
  }

  .hidden-input {
    position: fixed;
    bottom: 48px;
    left: 0;
    width: 100%;
    height: 44px;
    opacity: 0.01;
    z-index: 5;
    font-size: 16px; /* prevents iOS zoom */
    background: transparent;
    border: none;
    color: transparent;
    caret-color: transparent;
  }
</style>
