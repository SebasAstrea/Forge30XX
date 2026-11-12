<script lang="ts">
  // Vista de chat con streaming (RF-18) + panel de aprobación (RF-20).
  import { chat, decideTool, type Message, type ToolRequest, type SseEvent } from "./api";
  import { connection } from "./connection";
  import ToolPanel from "./ToolPanel.svelte";

  let messages: Message[] = [];
  let draft = "";
  let streaming = "";
  let busy = false;
  let pending: ToolRequest | null = null;

  function apply(event: SseEvent): void {
    if (event.kind === "token") streaming += event.text;
    else if (event.kind === "tool_request") pending = event.request;
    else if (event.kind === "tool_result") pending = null;
    else if (event.kind === "error") streaming += `\n[error] ${event.message}`;
  }

  async function send(): Promise<void> {
    const text = draft.trim();
    if (!text || busy) return;
    messages = [...messages, { role: "user", content: text }];
    draft = "";
    busy = true;
    streaming = "";
    await chat(messages, apply);
    messages = [...messages, { role: "assistant", content: streaming }];
    streaming = "";
    busy = false;
  }

  async function decide(id: string, decision: "approve" | "reject"): Promise<void> {
    await decideTool(id, decision);
    pending = null;
  }
</script>

<div class="status" data-state={$connection}>
  Conexión: {$connection}
</div>

<ul class="log" aria-live="polite">
  {#each messages as m}
    <li class={m.role}><span class="who">{m.role}</span>{m.content}</li>
  {/each}
  {#if streaming}
    <li class="assistant"><span class="who">assistant</span>{streaming}</li>
  {/if}
</ul>

{#if pending}
  <ToolPanel request={pending} onDecide={decide} />
{/if}

<form on:submit|preventDefault={send}>
  <label class="sr-only" for="prompt">Mensaje</label>
  <input
    id="prompt"
    bind:value={draft}
    placeholder="Escribe un mensaje…"
    disabled={busy}
    autocomplete="off"
  />
  <button type="submit" disabled={busy || !draft.trim()}>Enviar</button>
</form>

<style>
  .status {
    font-size: 0.85rem;
    padding: 0.25rem 0;
  }
  .status[data-state="online"] {
    color: #3c6;
  }
  .status[data-state="offline"] {
    color: #c44;
  }
  .status[data-state="connecting"] {
    color: #ca4;
  }
  .log {
    list-style: none;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .log li {
    white-space: pre-wrap;
    padding: 0.5rem;
    border-radius: 6px;
  }
  .log .user {
    background: #1b2a3a;
  }
  .log .assistant {
    background: #1e1e1e;
  }
  .who {
    display: block;
    font-size: 0.7rem;
    opacity: 0.6;
    text-transform: uppercase;
  }
  form {
    display: flex;
    gap: 0.5rem;
    margin-top: 1rem;
  }
  input {
    flex: 1;
    padding: 0.5rem;
    border-radius: 6px;
    border: 1px solid #444;
    background: #111;
    color: inherit;
  }
  button {
    padding: 0.5rem 1rem;
    border-radius: 6px;
    border: none;
    background: #176;
    color: #fff;
    cursor: pointer;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
