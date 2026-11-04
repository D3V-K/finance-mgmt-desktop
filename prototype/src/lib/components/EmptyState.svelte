<script lang="ts">
  import type { Snippet } from 'svelte';
  interface Props { title: string; body: string; action?: Snippet }
  let { title, body, action }: Props = $props();
</script>

<p class="title">{title}</p>
<p class="muted body">{body}</p>
{#if action}{@render action()}{/if}

<style>
  .title { font-weight: 620; }
  .body { max-width: 52ch; }
</style>
