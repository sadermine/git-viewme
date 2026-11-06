<script lang="ts">
  import '../app.css';
  import Shell from '$lib/components/Shell.svelte';
  import SignIn from '$lib/components/SignIn.svelte';
  import { session } from '$lib/session.svelte';

  void session.check();
</script>

{#if session.current.state === 'signed_in'}
  <Shell />
{:else if session.current.state === 'signed_out'}
  <SignIn />
{/if}
