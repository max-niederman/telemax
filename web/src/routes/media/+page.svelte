<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '$lib/api.svelte';
  import type { MediaState, PlayerInfo } from '$lib/types';

  // --- State: set ONLY by WebSocket, never by actions ---
  let media = $state<MediaState>({ status: 'stopped' });
  let players = $state<PlayerInfo[]>([]);
  let audioBands = $state<number[]>([0, 0, 0, 0, 0, 0, 0, 0, 0]);

  // --- Local interaction state ---
  let seeking = $state(false);
  let seekPosition = $state(0);
  let volumeDragging = $state(false);
  let localVolume = $state(0.5);

  // --- Position interpolation ---
  let serverPosition = $state(0);     // last position from server
  let serverTimestamp = $state(0);     // when we received it (performance.now())
  let interpolatedPosition = $state(0);
  let rafId: number | null = null;

  let cleanups: (() => void)[] = [];

  // --- Derived ---
  let isPlaying = $derived(media.status === 'Playing' || media.status === 'playing');
  let duration = $derived(media.duration_ms ?? 0);
  let displayPosition = $derived(seeking ? seekPosition : interpolatedPosition);
  let progress = $derived(duration > 0 ? (displayPosition / duration) * 100 : 0);
  let displayVolume = $derived(volumeDragging ? localVolume : (media.volume ?? 0.5));
  let shuffleOn = $derived(media.shuffle === true);
  let repeatMode = $derived(media.repeat ?? 'None');

  function formatTime(ms: number): string {
    const totalSeconds = Math.floor(Math.max(0, ms) / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  }

  // --- Position interpolation loop ---
  function tick() {
    if (isPlaying && !seeking) {
      const elapsed = performance.now() - serverTimestamp;
      interpolatedPosition = serverPosition + elapsed;
      if (duration > 0) {
        interpolatedPosition = Math.min(interpolatedPosition, duration);
      }
    }
    rafId = requestAnimationFrame(tick);
  }

  // --- Actions: fire-and-forget, state comes back via WebSocket ---
  function action(name: string) {
    api.post(`/media/${name}`).catch(() => {});
  }

  function skip(seconds: number) {
    const pos = Math.max(0, Math.min(duration, displayPosition + seconds * 1000));
    // Immediately update interpolation base so it feels instant
    serverPosition = pos;
    serverTimestamp = performance.now();
    interpolatedPosition = pos;
    api.post('/media/seek', { position_ms: Math.round(pos) }).catch(() => {});
  }

  function selectPlayer(id: string) {
    api.post('/media/player', { id }).catch(() => {});
  }

  function toggleShuffle() {
    api.post('/media/shuffle').catch(() => {});
  }

  function toggleRepeat() {
    api.post('/media/repeat').catch(() => {});
  }

  // --- Seek interaction ---
  function handleSeekDown(e: PointerEvent) {
    if (!duration) return;
    seeking = true;
    updateSeekFromEvent(e);
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function handleSeekMove(e: PointerEvent) {
    if (!seeking) return;
    updateSeekFromEvent(e);
  }

  function handleSeekUp() {
    if (!seeking) return;
    seeking = false;
    serverPosition = seekPosition;
    serverTimestamp = performance.now();
    interpolatedPosition = seekPosition;
    api.post('/media/seek', { position_ms: Math.round(seekPosition) }).catch(() => {});
  }

  function updateSeekFromEvent(e: PointerEvent) {
    const bar = e.currentTarget as HTMLElement;
    const rect = bar.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    seekPosition = ratio * duration;
  }

  // --- Volume interaction ---
  function handleVolumeInput(e: Event) {
    const value = parseFloat((e.target as HTMLInputElement).value);
    localVolume = value;
    volumeDragging = true;
    api.post('/media/volume', { volume: value }).catch(() => {});
  }

  function handleVolumeChange() {
    // Release — stop overriding server value after a short delay
    setTimeout(() => { volumeDragging = false; }, 1000);
  }

  // --- Lifecycle ---
  onMount(() => {
    rafId = requestAnimationFrame(tick);

    // WebSocket is the single source of truth
    cleanups.push(
      api.on('media_state', (msg: any) => {
        // Update players list
        if (Array.isArray(msg.players)) {
          players = msg.players;
        }

        // Update media state (everything except position handled here)
        const prev = media;
        media = {
          status: msg.status ?? 'stopped',
          title: msg.title,
          artist: msg.artist,
          album: msg.album,
          art_url: msg.art_url,
          position_ms: msg.position_ms,
          duration_ms: msg.duration_ms,
          volume: msg.volume,
          shuffle: msg.shuffle,
          repeat: msg.repeat,
          player_id: msg.player_id,
        };

        // Sync position interpolation base (unless user is seeking)
        if (!seeking && msg.position_ms != null) {
          serverPosition = msg.position_ms;
          serverTimestamp = performance.now();
          interpolatedPosition = msg.position_ms;
        }
      }),

      api.on('audio_level', (msg: any) => {
        if (Array.isArray(msg.bands)) {
          audioBands = msg.bands.map((v: number) => Math.max(0, Math.min(1, v ?? 0)));
        }
      })
    );
  });

  onDestroy(() => {
    if (rafId !== null) cancelAnimationFrame(rafId);
    cleanups.forEach((fn) => fn());
  });
</script>

<div class="media-page">
  <!-- Player selector -->
  {#if players.length > 1}
    <div class="player-selector">
      {#each players as player}
        <button
          class="player-tab"
          class:active={player.id === media.player_id}
          onclick={() => selectPlayer(player.id)}
        >
          {player.name.toUpperCase()}
        </button>
      {/each}
    </div>
  {/if}

  <!-- Track info -->
  <div class="track-info">
    <div class="track-title">{media.title || 'NO MEDIA PLAYING'}</div>
    {#if media.artist}
      <div class="track-artist">{media.artist}</div>
    {/if}
    {#if media.album}
      <div class="track-album">{media.album}</div>
    {/if}
  </div>

  <!-- Progress bar -->
  {#if duration > 0}
    <div class="progress-section">
      <div
        class="progress-bar"
        role="slider"
        tabindex="0"
        aria-label="Seek"
        aria-valuemin={0}
        aria-valuemax={duration}
        aria-valuenow={displayPosition}
        onpointerdown={handleSeekDown}
        onpointermove={handleSeekMove}
        onpointerup={handleSeekUp}
      >
        <div class="progress-track">
          <div class="progress-fill" style="width: {progress}%"></div>
        </div>
      </div>
      <div class="time-labels">
        <span>{formatTime(displayPosition)}</span>
        <span>{formatTime(duration)}</span>
      </div>
    </div>
  {:else if displayPosition > 0}
    <div class="progress-section">
      <div class="time-labels" style="justify-content: center">
        <span>{formatTime(displayPosition)}</span>
      </div>
    </div>
  {/if}

  <!-- Transport controls -->
  <div class="transport">
    {#if media.shuffle != null}
    <button class="transport-text small" class:toggled={shuffleOn} onclick={toggleShuffle} aria-label="Shuffle">
      SHUF
    </button>
    {/if}

    <button class="transport-text" onclick={() => action('prev')} aria-label="Previous">
      &#x27E8;&#x27E8;
    </button>

    <button class="transport-text skip" onclick={() => skip(-15)} aria-label="Back 15s">
      &minus;15
    </button>

    <button class="transport-text play" onclick={() => action(isPlaying ? 'pause' : 'play')} aria-label={isPlaying ? 'Pause' : 'Play'}>
      {#if isPlaying}<span class="pause-icon"></span>{:else}<span class="play-icon"></span>{/if}
    </button>

    <button class="transport-text skip" onclick={() => skip(15)} aria-label="Forward 15s">
      +15
    </button>

    <button class="transport-text" onclick={() => action('next')} aria-label="Next">
      &#x27E9;&#x27E9;
    </button>

    {#if media.repeat != null}
    <button class="transport-text small" class:toggled={repeatMode !== 'None'} onclick={toggleRepeat} aria-label="Repeat">
      RPT{#if repeatMode === 'Track'}&middot;1{:else if repeatMode === 'Playlist'}&middot;A{/if}
    </button>
    {/if}
  </div>

  <!-- Volume slider -->
  {#if media.volume != null}
  <div class="volume-section">
    <span class="volume-label">VOL</span>
    <input
      type="range"
      class="volume-slider"
      min="0"
      max="1"
      step="0.01"
      value={displayVolume}
      oninput={handleVolumeInput}
      onchange={handleVolumeChange}
      aria-label="Volume"
    />
    <span class="volume-value">{Math.round(displayVolume * 100)}</span>
  </div>
  {/if}

  <!-- Audio spectrum visualizer -->
  <div class="spectrum">
    <div class="spectrum-bars">
      {#each audioBands as level}
        <div class="spectrum-col">
          <div class="spectrum-bar-wrap">
            <div
              class="spectrum-bar"
              style="transform: scaleY({Math.max(0.02, level)})"
            ></div>
          </div>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .media-page {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 16px 16px 24px;
    overflow-y: auto;
    gap: 16px;
    touch-action: pan-y;
  }

  .player-selector {
    display: flex;
    gap: 0;
    overflow-x: auto;
    overflow-y: hidden;
    flex-shrink: 0;
    border-bottom: 1px solid #333333;
    scrollbar-width: none;
  }

  .player-selector::-webkit-scrollbar {
    display: none;
  }

  .player-tab {
    padding: 10px 16px;
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

  .player-tab.active {
    color: #ffffff;
    border-bottom-color: #ff2d2d;
  }

  .track-info {
    text-align: left;
    flex-shrink: 0;
  }

  .track-title {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 20px;
    font-weight: 700;
    color: #ffffff;
    margin-bottom: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .track-artist {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 14px;
    font-weight: 300;
    color: #666666;
    margin-bottom: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .track-album {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 13px;
    font-weight: 300;
    color: #333333;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .progress-section {
    flex-shrink: 0;
  }

  .progress-bar {
    position: relative;
    padding: 12px 0;
    cursor: pointer;
    touch-action: none;
  }

  .progress-track {
    height: 1px;
    background: #333333;
    position: relative;
    overflow: visible;
  }

  .progress-fill {
    height: 100%;
    background: #ffffff;
  }

  .time-labels {
    display: flex;
    justify-content: space-between;
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 11px;
    color: #666666;
    margin-top: 4px;
  }

  .transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0;
    flex-shrink: 0;
    border-top: 1px solid #333333;
    border-bottom: 1px solid #333333;
  }

  .transport-text {
    border: none;
    background: transparent;
    color: #ffffff;
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 14px;
    font-weight: 500;
    letter-spacing: 0.05em;
    cursor: pointer;
    padding: 16px 12px;
    min-width: 48px;
    text-align: center;
  }

  .transport-text:active {
    color: #ff2d2d;
  }

  .transport-text.play {
    padding: 12px 20px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .play-icon {
    width: 0;
    height: 0;
    border-style: solid;
    border-width: 14px 0 14px 22px;
    border-color: transparent transparent transparent #ffffff;
    display: block;
  }

  .pause-icon {
    width: 22px;
    height: 28px;
    border-left: 6px solid #ffffff;
    border-right: 6px solid #ffffff;
    display: block;
  }

  .transport-text.skip {
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 12px;
    font-weight: 500;
    color: #666666;
  }

  .transport-text.skip:active {
    color: #ff2d2d;
  }

  .transport-text.small {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: #333333;
  }

  .transport-text.toggled {
    color: #ff2d2d;
  }

  .volume-section {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
  }

  .volume-label {
    font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: #666666;
    flex-shrink: 0;
  }

  .volume-value {
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 11px;
    color: #666666;
    flex-shrink: 0;
    min-width: 24px;
    text-align: right;
  }

  .volume-slider {
    flex: 1;
    -webkit-appearance: none;
    appearance: none;
    height: 1px;
    background: #333333;
    outline: none;
  }

  .volume-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    background: #ffffff;
    cursor: pointer;
  }

  .volume-slider::-moz-range-thumb {
    width: 12px;
    height: 12px;
    background: #ffffff;
    border: none;
    cursor: pointer;
  }

  .spectrum {
    flex-shrink: 0;
    padding: 0 0 8px;
  }

  .spectrum-bars {
    display: flex;
    align-items: flex-end;
    gap: 6px;
    height: 64px;
  }

  .spectrum-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    height: 100%;
  }

  .spectrum-bar-wrap {
    flex: 1;
    width: 1px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }

  .spectrum-bar {
    width: 1px;
    height: 100%;
    background: #ffffff;
    transform-origin: bottom;
    transition: transform 50ms linear;
    min-height: 1px;
  }
</style>
