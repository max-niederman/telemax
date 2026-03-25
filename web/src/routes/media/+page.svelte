<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '$lib/api.svelte';
  import { base } from '$app/paths';
  import type { MediaState, PlayerInfo } from '$lib/types';

  let media = $state<MediaState>({
    status: 'stopped',
  });
  let players = $state<PlayerInfo[]>([]);
  let audioBands = $state<number[]>([0, 0, 0, 0, 0, 0, 0, 0, 0]);
  const bandLabels = ['60', '150', '400', '1k', '2.5k', '6k', '12k', '16k', '20k'];
  let artError = $state(false);
  let artUrl = $state('');
  let seeking = $state(false);
  let seekPosition = $state(0);
  let pollTimer: ReturnType<typeof setInterval>;
  let cleanups: (() => void)[] = [];

  // Derived
  let isPlaying = $derived(media.status === 'Playing' || media.status === 'playing');
  let progress = $derived(
    media.duration_ms && media.duration_ms > 0
      ? ((seeking ? seekPosition : (media.position_ms ?? 0)) / media.duration_ms) * 100
      : 0
  );
  let elapsed = $derived(seeking ? seekPosition : (media.position_ms ?? 0));
  let duration = $derived(media.duration_ms ?? 0);
  let shuffleOn = $derived(media.shuffle === true);
  let repeatMode = $derived(media.repeat ?? 'None');

  function formatTime(ms: number): string {
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  }

  async function fetchMedia() {
    try {
      const state = await api.get<MediaState>('/media');
      media = state;
      artError = false;
      artUrl = `${base}/api/media/art?t=${Date.now()}`;
    } catch {
      // ignore
    }
  }

  async function fetchPlayers() {
    try {
      players = await api.get<PlayerInfo[]>('/media/players');
    } catch {
      // ignore
    }
  }

  async function transport(action: string) {
    // Optimistic UI update
    if (action === 'play') media.status = 'Playing';
    else if (action === 'pause') media.status = 'Paused';
    try {
      await api.post(`/media/${action}`);
      await fetchMedia();
    } catch {
      // ignore
    }
  }

  async function skip(seconds: number) {
    const newPos = Math.max(0, Math.min(duration, (media.position_ms ?? 0) + seconds * 1000));
    media.position_ms = newPos;
    try {
      await api.post('/media/seek', { position_ms: newPos });
    } catch {
      // ignore
    }
  }

  async function toggleShuffle() {
    media.shuffle = !media.shuffle;
    try {
      await api.post('/media/shuffle');
    } catch {
      // ignore
    }
  }

  async function toggleRepeat() {
    const cycle: Record<string, string> = { 'None': 'Track', 'Track': 'Playlist', 'Playlist': 'None' };
    media.repeat = cycle[media.repeat ?? 'None'] ?? 'None';
    try {
      await api.post('/media/repeat');
    } catch {
      // ignore
    }
  }

  let localVolume = $state<number | null>(null);
  let volumeTimeout: ReturnType<typeof setTimeout> | undefined;
  let displayVolume = $derived(localVolume ?? media.volume ?? 0.5);

  async function setVolume(vol: number) {
    localVolume = vol;
    clearTimeout(volumeTimeout);
    volumeTimeout = setTimeout(() => { localVolume = null; }, 2000);
    try {
      await api.post('/media/volume', { volume: vol });
    } catch {
      // ignore
    }
  }

  async function seekTo(posMs: number) {
    try {
      await api.post('/media/seek', { position_ms: posMs });
    } catch {
      // ignore
    }
  }

  async function selectPlayer(id: string) {
    try {
      await api.post('/media/player', { id });
      await fetchMedia();
    } catch {
      // ignore
    }
  }

  function handleProgressPointerDown(e: PointerEvent) {
    if (!duration) return;
    seeking = true;
    updateSeekFromEvent(e);
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function handleProgressPointerMove(e: PointerEvent) {
    if (!seeking) return;
    updateSeekFromEvent(e);
  }

  function handleProgressPointerUp(_e: PointerEvent) {
    if (!seeking) return;
    seeking = false;
    seekTo(seekPosition);
  }

  function updateSeekFromEvent(e: PointerEvent) {
    const bar = e.currentTarget as HTMLElement;
    const rect = bar.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    seekPosition = ratio * duration;
  }

  function handleVolumeInput(e: Event) {
    const value = parseFloat((e.target as HTMLInputElement).value);
    setVolume(value);
  }

  onMount(() => {
    fetchMedia();
    fetchPlayers();

    pollTimer = setInterval(fetchMedia, 5000);

    cleanups.push(
      api.on('media_changed', (msg) => {
        if (msg.state) {
          media = msg.state;
          artError = false;
          artUrl = `${base}/api/media/art?t=${Date.now()}`;
        }
      }),
      api.on('media_progress', (msg) => {
        if (!seeking && msg.position_ms !== undefined) {
          media.position_ms = msg.position_ms;
        }
      }),
      api.on('audio_level', (msg) => {
        if (Array.isArray(msg.bands)) {
          audioBands = msg.bands.map((v: number) => Math.max(0, Math.min(1, v ?? 0)));
        }
      })
    );
  });

  onDestroy(() => {
    clearInterval(pollTimer);
    cleanups.forEach((fn) => fn());
  });
</script>

<div class="media-page">
  <!-- Player selector -->
  {#if players.length > 1}
    <div class="player-selector">
      {#each players as player}
        <button
          class="player-pill"
          class:active={player.id === media.player_id}
          onclick={() => selectPlayer(player.id)}
        >
          {player.name}
        </button>
      {/each}
    </div>
  {/if}

  <!-- Cover art -->
  <div class="art-container">
    {#if artUrl && !artError}
      <img
        src={artUrl}
        alt="Album art"
        class="cover-art"
        onerror={() => { artError = true; }}
      />
    {:else}
      <div class="art-placeholder">
        <svg viewBox="0 0 80 80" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="40" cy="40" r="30" />
          <circle cx="40" cy="40" r="8" />
          <path d="M52 20 L52 44" stroke-width="3" />
          <path d="M52 20 L62 24 L62 18 Z" fill="currentColor" stroke="none" />
        </svg>
      </div>
    {/if}
  </div>

  <!-- Track info -->
  <div class="track-info">
    <div class="track-title">{media.title || 'No media playing'}</div>
    <div class="track-artist">{media.artist || ''}</div>
    <div class="track-album">{media.album || ''}</div>
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
        aria-valuenow={elapsed}
        onpointerdown={handleProgressPointerDown}
        onpointermove={handleProgressPointerMove}
        onpointerup={handleProgressPointerUp}
      >
        <div class="progress-track">
          <div class="progress-fill" style="width: {progress}%"></div>
          <div class="progress-thumb" style="left: {progress}%"></div>
        </div>
      </div>
      <div class="time-labels">
        <span>{formatTime(elapsed)}</span>
        <span>{formatTime(duration)}</span>
      </div>
    </div>
  {:else if elapsed > 0}
    <div class="progress-section">
      <div class="time-labels" style="justify-content: center">
        <span>{formatTime(elapsed)}</span>
      </div>
    </div>
  {/if}

  <!-- Transport controls -->
  <div class="transport">
    <button class="transport-btn small" class:toggled={shuffleOn} onclick={toggleShuffle} aria-label="Shuffle">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="16,3 21,3 21,8" />
        <line x1="4" y1="20" x2="21" y2="3" />
        <polyline points="21,16 21,21 16,21" />
        <line x1="15" y1="15" x2="21" y2="21" />
        <line x1="4" y1="4" x2="9" y2="9" />
      </svg>
    </button>

    <button class="transport-btn" onclick={() => transport('prev')} aria-label="Previous">
      <svg viewBox="0 0 24 24" fill="currentColor" stroke="none">
        <path d="M6 6h2v12H6zM9.5 12l8.5 6V6z" />
      </svg>
    </button>

    <button class="transport-btn skip" onclick={() => skip(-15)} aria-label="Back 15s">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M1 4v6h6" />
        <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
        <text x="12" y="15.5" text-anchor="middle" font-size="7" fill="currentColor" stroke="none" font-weight="bold">15</text>
      </svg>
    </button>

    <button class="transport-btn play" onclick={() => transport(isPlaying ? 'pause' : 'play')} aria-label={isPlaying ? 'Pause' : 'Play'}>
      {#if isPlaying}
        <svg viewBox="0 0 24 24" fill="currentColor" stroke="none">
          <rect x="6" y="4" width="4" height="16" rx="1" />
          <rect x="14" y="4" width="4" height="16" rx="1" />
        </svg>
      {:else}
        <svg viewBox="0 0 24 24" fill="currentColor" stroke="none">
          <polygon points="6,4 20,12 6,20" />
        </svg>
      {/if}
    </button>

    <button class="transport-btn skip" onclick={() => skip(15)} aria-label="Forward 15s">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M23 4v6h-6" />
        <path d="M20.49 15a9 9 0 1 1-2.13-9.36L23 10" />
        <text x="12" y="15.5" text-anchor="middle" font-size="7" fill="currentColor" stroke="none" font-weight="bold">15</text>
      </svg>
    </button>

    <button class="transport-btn" onclick={() => transport('next')} aria-label="Next">
      <svg viewBox="0 0 24 24" fill="currentColor" stroke="none">
        <path d="M16 6h2v12h-2zM6 18l8.5-6L6 6z" />
      </svg>
    </button>

    <button class="transport-btn small" class:toggled={repeatMode !== 'None'} onclick={toggleRepeat} aria-label="Repeat">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="17,1 21,5 17,9" />
        <path d="M3 11V9a4 4 0 0 1 4-4h14" />
        <polyline points="7,23 3,19 7,15" />
        <path d="M21 13v2a4 4 0 0 1-4 4H3" />
        {#if repeatMode === 'Track'}
          <text x="12" y="15" text-anchor="middle" font-size="8" fill="currentColor" stroke="none">1</text>
        {/if}
      </svg>
    </button>
  </div>

  <!-- Volume slider -->
  {#if media.volume != null}
  <div class="volume-section">
    <svg class="volume-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <polygon points="11,5 6,9 2,9 2,15 6,15 11,19" fill="currentColor" />
      {#if displayVolume > 0}
        <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
      {/if}
      {#if displayVolume > 0.5}
        <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
      {/if}
    </svg>
    <input
      type="range"
      class="volume-slider"
      min="0"
      max="1"
      step="0.01"
      value={displayVolume}
      oninput={handleVolumeInput}
      aria-label="Volume"
    />
  </div>
  {/if}

  <!-- Audio spectrum visualizer -->
  <div class="spectrum">
    <div class="spectrum-bars">
      {#each audioBands as level, i}
        <div class="spectrum-col">
          <div class="spectrum-bar-wrap">
            <div
              class="spectrum-bar"
              style="transform: scaleY({Math.max(0.02, level)}); --band-hue: {30 + i * 25}"
            ></div>
          </div>
          <span class="spectrum-label">{bandLabels[i]}</span>
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
    padding: 12px 16px 24px;
    overflow-y: auto;
    gap: 12px;
    touch-action: pan-y;
  }

  /* Player selector */
  .player-selector {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 4px 0;
    flex-shrink: 0;
  }

  .player-pill {
    padding: 6px 16px;
    border-radius: 20px;
    border: 1px solid #0f3460;
    background: #16213e;
    color: #888;
    font-size: 13px;
    white-space: nowrap;
    cursor: pointer;
    transition: all 0.2s;
    flex-shrink: 0;
  }

  .player-pill.active {
    background: #7c3aed;
    border-color: #7c3aed;
    color: #fff;
  }

  /* Cover art */
  .art-container {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    aspect-ratio: 1;
    max-height: 40vh;
    align-self: center;
    width: 100%;
    max-width: 320px;
  }

  .cover-art {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 16px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .art-placeholder {
    width: 100%;
    height: 100%;
    background: #16213e;
    border-radius: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #333;
  }

  .art-placeholder svg {
    width: 80px;
    height: 80px;
  }

  /* Track info */
  .track-info {
    text-align: center;
    flex-shrink: 0;
  }

  .track-title {
    font-size: 20px;
    font-weight: 700;
    color: #e2e8f0;
    margin-bottom: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .track-artist {
    font-size: 15px;
    color: #94a3b8;
    margin-bottom: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .track-album {
    font-size: 13px;
    color: #64748b;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Progress bar */
  .progress-section {
    flex-shrink: 0;
    padding: 0 4px;
  }

  .progress-bar {
    position: relative;
    padding: 10px 0;
    cursor: pointer;
    touch-action: none;
  }

  .progress-track {
    height: 4px;
    background: #16213e;
    border-radius: 2px;
    position: relative;
    overflow: visible;
  }

  .progress-fill {
    height: 100%;
    background: #7c3aed;
    border-radius: 2px;
    transition: width 0.1s linear;
  }

  .progress-thumb {
    position: absolute;
    top: 50%;
    width: 14px;
    height: 14px;
    background: #7c3aed;
    border-radius: 50%;
    transform: translate(-50%, -50%);
    box-shadow: 0 2px 6px rgba(124, 58, 237, 0.4);
    transition: left 0.1s linear;
  }

  .time-labels {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    color: #64748b;
    margin-top: 4px;
  }

  /* Transport controls */
  .transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 16px;
    flex-shrink: 0;
    padding: 4px 0;
  }

  .transport-btn {
    width: 48px;
    height: 48px;
    border: none;
    background: transparent;
    color: #e2e8f0;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    border-radius: 50%;
    transition: all 0.15s;
  }

  .transport-btn:active {
    transform: scale(0.9);
    background: rgba(124, 58, 237, 0.15);
  }

  .transport-btn svg {
    width: 28px;
    height: 28px;
  }

  .transport-btn.small {
    width: 40px;
    height: 40px;
  }

  .transport-btn.small svg {
    width: 20px;
    height: 20px;
  }

  .transport-btn.skip {
    width: 40px;
    height: 40px;
  }

  .transport-btn.skip svg {
    width: 24px;
    height: 24px;
  }

  .transport-btn.play {
    width: 64px;
    height: 64px;
    background: #7c3aed;
    border-radius: 50%;
    box-shadow: 0 4px 16px rgba(124, 58, 237, 0.4);
  }

  .transport-btn.play:active {
    background: #6d28d9;
    transform: scale(0.95);
  }

  .transport-btn.play svg {
    width: 32px;
    height: 32px;
  }

  .transport-btn.toggled {
    color: #7c3aed;
  }

  /* Volume */
  .volume-section {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
    padding: 0 4px;
  }

  .volume-icon {
    width: 22px;
    height: 22px;
    color: #94a3b8;
    flex-shrink: 0;
  }

  .volume-slider {
    flex: 1;
    -webkit-appearance: none;
    appearance: none;
    height: 4px;
    background: #16213e;
    border-radius: 2px;
    outline: none;
  }

  .volume-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 18px;
    height: 18px;
    background: #7c3aed;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 2px 6px rgba(124, 58, 237, 0.3);
  }

  .volume-slider::-moz-range-thumb {
    width: 18px;
    height: 18px;
    background: #7c3aed;
    border: none;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 2px 6px rgba(124, 58, 237, 0.3);
  }

  /* Spectrum visualizer */
  .spectrum {
    flex-shrink: 0;
    padding: 0 4px 8px;
  }

  .spectrum-bars {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 80px;
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
    width: 100%;
    display: flex;
    align-items: flex-end;
  }

  .spectrum-bar {
    width: 100%;
    height: 100%;
    border-radius: 3px 3px 0 0;
    background: hsl(var(--band-hue), 70%, 55%);
    transform-origin: bottom;
    transition: transform 50ms linear;
    min-height: 2px;
  }

  .spectrum-label {
    font-size: 9px;
    color: #64748b;
    margin-top: 4px;
    white-space: nowrap;
  }
</style>
