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
      // Seed with a space so backspace has something to act on
      if (hiddenInput) {
        hiddenInput.value = ' ';
        hiddenInput.setSelectionRange(1, 1);
      }
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
    // Pass through key names that the server keymap already handles directly.
    // The server expects JavaScript KeyboardEvent.key values.
    const suppress: string[] = ['Control', 'Alt', 'Shift', 'Meta'];
    if (suppress.includes(key)) return null;
    if (key.length === 1) return key; // letters, digits, space, punctuation
    return key; // Enter, Backspace, Delete, ArrowUp, etc.
  }

  // Handle input events for mobile keyboards that don't fire proper key events
  function handleInput(e: Event) {
    const input = e.target as HTMLInputElement;
    const value = input.value;
    // Only send characters beyond the seeded space placeholder
    const newChars = value.replace(/^ /, '');
    if (newChars) {
      for (const char of newChars) {
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
    }
    // Reset to just the placeholder space
    input.value = ' ';
    input.setSelectionRange(1, 1);
  }

  // Handle beforeinput for delete/backspace on mobile keyboards
  function handleBeforeInput(e: InputEvent) {
    if (e.inputType === 'deleteContentBackward') {
      e.preventDefault();
      api.send({ type: 'key_press', key: 'Backspace', modifiers: getActiveModifiers() });
      api.send({ type: 'key_release', key: 'Backspace', modifiers: getActiveModifiers() });
    } else if (e.inputType === 'deleteContentForward') {
      e.preventDefault();
      api.send({ type: 'key_press', key: 'Delete', modifiers: getActiveModifiers() });
      api.send({ type: 'key_release', key: 'Delete', modifiers: getActiveModifiers() });
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
    onbeforeinput={handleBeforeInput}
    onblur={() => { keyboardOpen = false; }}
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
