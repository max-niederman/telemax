<script lang="ts">
  import { goto } from '$app/navigation';
  import { base } from '$app/paths';
  import { onMount } from 'svelte';

  onMount(() => {
    goto(`${base}/trackpad`, { replaceState: true });
  });
</script>

<div class="loading">
  <p>telemax</p>
</div>

<style>
  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #555555;
    font-size: 14px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
</style>
