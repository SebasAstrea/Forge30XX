// Cliente de la API local del orquestador (ADR-0015, docs/openapi.yaml).
// El chat usa SSE sobre `fetch` porque el endpoint es POST con cuerpo
// (EventSource solo admite GET). Los tipos reflejan el esquema de eventos.

export type Role = "system" | "user" | "assistant";

export interface Message {
  role: Role;
  content: string;
}

export interface ToolRequest {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  destructive: boolean;
}

export type SseEvent =
  | { kind: "token"; text: string }
  | { kind: "tool_request"; request: ToolRequest }
  | { kind: "tool_result"; id: string; status: string }
  | { kind: "error"; message: string }
  | { kind: "done" };

type Handler = (sseEvent: SseEvent) => void;

// Construye el SseEvent tipado a partir del nombre de evento y el JSON.
const EVENT_BUILDERS: Record<string, (json: Record<string, unknown>) => SseEvent> = {
  token: (j) => ({ kind: "token", text: j.text as string }),
  tool_request: (j) => ({ kind: "tool_request", request: j as unknown as ToolRequest }),
  tool_result: (j) => ({ kind: "tool_result", id: j.id as string, status: j.status as string }),
  error: (j) => ({ kind: "error", message: j.message as string }),
  done: () => ({ kind: "done" }),
};

// Convierte un bloque SSE ("event: x\ndata: {...}") en un SseEvent tipado.
export function parseBlock(block: string): SseEvent | null {
  let name = "";
  let data = "";
  for (const line of block.split("\n")) {
    if (line.startsWith("event:")) name = line.slice(6).trim();
    else if (line.startsWith("data:")) data += line.slice(5).trim();
  }
  const build = EVENT_BUILDERS[name];
  if (!build) return null;
  return build(data ? JSON.parse(data) : {});
}

// Lee el cuerpo SSE trozo a trozo y entrega eventos completos a `onEvent`.
async function readSse(body: ReadableStream<Uint8Array>, onEvent: Handler): Promise<void> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    let sep = buffer.indexOf("\n\n");
    while (sep !== -1) {
      const event = parseBlock(buffer.slice(0, sep));
      if (event) onEvent(event);
      buffer = buffer.slice(sep + 2);
      sep = buffer.indexOf("\n\n");
    }
  }
}

// Inicia un turno de chat; invoca `onEvent` por cada evento del stream.
export async function chat(messages: Message[], onEvent: Handler): Promise<void> {
  const resp = await fetch("/api/chat", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ messages }),
  });
  if (!resp.ok || !resp.body) {
    onEvent({ kind: "error", message: `HTTP ${resp.status}` });
    return;
  }
  await readSse(resp.body, onEvent);
}

// Resuelve un tool-call destructivo pendiente (RF-20).
export async function decideTool(id: string, decision: "approve" | "reject"): Promise<string> {
  const resp = await fetch(`/api/tools/${id}/decision`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ decision }),
  });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const json = await resp.json();
  return json.status as string;
}

// Consulta la salud del llama-server para el indicador de conexión.
export async function health(): Promise<boolean> {
  try {
    const resp = await fetch("/api/health");
    if (!resp.ok) return false;
    const json = await resp.json();
    return json.llama_server === true;
  } catch {
    return false;
  }
}
