// Store de estado de conexión con el backend (AGENTS.md 11.3: mostrar estado
// y reconectar con backoff exponencial). Sondea /api/health y espacia los
// reintentos cuando falla, hasta un máximo.

import { readable } from "svelte/store";
import { health } from "./api";

export type ConnectionState = "connecting" | "online" | "offline";

const MIN_DELAY_MS = 1000;
const MAX_DELAY_MS = 30000;

// Sondea la salud; el intervalo crece exponencialmente mientras esté offline
// y vuelve al mínimo al recuperar la conexión.
export const connection = readable<ConnectionState>("connecting", (set) => {
  let delay = MIN_DELAY_MS;
  let timer: ReturnType<typeof setTimeout>;
  let cancelled = false;

  async function tick(): Promise<void> {
    const ok = await health();
    if (cancelled) return;
    set(ok ? "online" : "offline");
    delay = ok ? MIN_DELAY_MS : Math.min(delay * 2, MAX_DELAY_MS);
    timer = setTimeout(tick, delay);
  }

  tick();
  return () => {
    cancelled = true;
    clearTimeout(timer);
  };
});
