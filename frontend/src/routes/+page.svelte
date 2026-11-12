<script lang="ts">
  // UI local de Forge30XX (S9, RF-18/RF-20). El chat y el panel de tool-calls
  // viven en componentes de $lib para respetar el límite de 150 líneas/comp.
  import Chat from "$lib/Chat.svelte";
</script>

<main>
  <h1>Forge30XX</h1>
  <Chat />
</main>

<style>
  main {
    max-width: 48rem;
    margin: 0 auto;
    padding: 1rem;
    font-family: system-ui, sans-serif;
  }
  h1 {
    font-size: 1.4rem;
  }
</style>
