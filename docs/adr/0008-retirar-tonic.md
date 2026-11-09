# ADR-0008: Retirar tonic del orquestador

- **Estado**: Aceptada
- **Fecha**: 2026-11-09
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: consecuencia abierta de ADR-0004
- **Modifica**: SDD §8 (Orquestador)

## Contexto

`tonic` (gRPC) estaba en el stack para hablar con la inferencia. ADR-0004 fijó
HTTP/OpenAI para la inferencia y ADR-0001/0002 HTTP para el RAG. Ningún
módulo usa `tonic`, pero se compila en cada build.

## Decisión

Se elimina `tonic` de `orchestrator/Cargo.toml` y del stack del SDD §8. Si un
componente futuro necesita gRPC, se añadirá con una ADR nueva.

## Alternativas consideradas

- **A — Retirarlo (elegida).** Menos dependencias y builds más cortos; YAGNI.
- **B — Mantenerlo por si se usa gRPC más adelante.** Coste de compilación sin
  uso y una dependencia más que auditar.

## Consecuencias

- `Cargo.lock` deja de incluir `tonic` y sus dependencias exclusivas.
- Sin cambios de comportamiento: no había código que lo usara.
