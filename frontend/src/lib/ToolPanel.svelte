<script lang="ts">
  // Panel de aprobación de tool-calls destructivas (RF-20, ADR-0015).
  // Sin aprobación no se ejecuta: muestra la tool pendiente y delega la
  // decisión al padre, que llama a la API.
  import type { ToolRequest } from "./api";

  export let request: ToolRequest;
  export let onDecide: (id: string, decision: "approve" | "reject") => void;

  $: args = JSON.stringify(request.arguments, null, 2);
</script>

<section class="panel" aria-labelledby="tool-title">
  <h2 id="tool-title">Aprobación requerida</h2>
  <p>
    El agente quiere ejecutar
    <strong>{request.name}</strong>{#if request.destructive}
      <span class="badge" aria-label="acción destructiva">destructiva</span>{/if}.
  </p>
  <pre aria-label="argumentos de la tool">{args}</pre>
  <div class="actions">
    <button type="button" class="approve" on:click={() => onDecide(request.id, "approve")}>
      Aprobar
    </button>
    <button type="button" class="reject" on:click={() => onDecide(request.id, "reject")}>
      Rechazar
    </button>
  </div>
</section>

<style>
  .panel {
    border: 1px solid #c04;
    border-radius: 8px;
    padding: 1rem;
    margin: 0.5rem 0;
    background: #2a1416;
  }
  .badge {
    background: #c04;
    color: #fff;
    border-radius: 4px;
    padding: 0 0.4rem;
    margin-left: 0.3rem;
    font-size: 0.8rem;
  }
  pre {
    background: #111;
    padding: 0.5rem;
    border-radius: 4px;
    overflow-x: auto;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
  }
  button {
    padding: 0.4rem 1rem;
    border-radius: 4px;
    border: none;
    cursor: pointer;
  }
  .approve {
    background: #176;
    color: #fff;
  }
  .reject {
    background: #633;
    color: #fff;
  }
  button:focus-visible {
    outline: 2px solid #fff;
    outline-offset: 2px;
  }
</style>
