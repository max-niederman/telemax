<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '$lib/api.svelte';
  import type { MediaState, PlayerInfo } from '$lib/types';

  let media = $state<MediaState>({
    status: 'stopped',
  });
  let players = $state<PlayerInfo[]>([]);
  let audioBands = $state<number[]>([0, 0, 0, 0, 0, 0, 0, 0, 0]);
  const bandLabels = ['60', '150', '400', '1k', '2.5k', '6k', '12k', '16k', '20k'];
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
    <div class="track-title">{media.title || 'No media playing'}</div>
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
        aria-valuenow={elapsed}
        onpointerdown={handleProgressPointerDown}
        onpointermove={handleProgressPointerMove}
        onpointerup={handleProgressPointerUp}
      >
        <div class="progress-track">
          <div class="progress-fill" style="width: {progress}%"></div>
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
    {#if media.shuffle != null}
    <button class="transport-text small" class:toggled={shuffleOn} onclick={toggleShuffle} aria-label="Shuffle">
      SHUF
    </button>
    {/if}

    <button class="transport-text" onclick={() => transport('prev')} aria-label="Previous">
      &#x27E8;&#x27E8;
    </button>

    <button class="transport-text skip" onclick={() => skip(-15)} aria-label="Back 15s">
      &minus;15
    </button>

    <button class="transport-text play" onclick={() => transport(isPlaying ? 'pause' : 'play')} aria-label={isPlaying ? 'Pause' : 'Play'}>
      {#if isPlaying}<span class="pause-icon"></span>{:else}<span class="play-icon"></span>{/if}
    </button>

    <button class="transport-text skip" onclick={() => skip(15)} aria-label="Forward 15s">
      +15
    </button>

    <button class="transport-text" onclick={() => transport('next')} aria-label="Next">
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
      aria-label="Volume"
    />
    <span class="volume-value">{Math.round(displayVolume * 100)}</span>
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

  /* Player selector */
  .player-selector {
    display: flex;
    gap: 0;
    overflow-x: auto;
    flex-shrink: 0;
    border-bottom: 1px solid #333333;
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

  /* Track info */
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

  /* Progress bar */
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
    transition: width 0.1s linear;
  }

  .time-labels {
    display: flex;
    justify-content: space-between;
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
    font-size: 11px;
    color: #666666;
    margin-top: 4px;
  }

  /* Transport controls */
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

  /* Volume */
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

  /* Spectrum visualizer */
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
