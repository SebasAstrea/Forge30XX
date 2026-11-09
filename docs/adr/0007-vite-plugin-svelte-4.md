# ADR-0007: Subir @sveltejs/vite-plugin-svelte a la versión 4

- **Estado**: Aceptada
- **Fecha**: 2026-11-09
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: deuda técnica del frontend (S9)
- **Modifica**: SDD §8 (Frontend)

## Contexto

El frontend usa Svelte 5, pero `@sveltejs/vite-plugin-svelte@3` solo soporta
Svelte 3/4 de forma oficial: `npm ci` avisa de peers incompatibles
(`svelte-hmr`). Funciona porque el frontend es un esqueleto.

## Decisión

Se sube a `@sveltejs/vite-plugin-svelte@^4.0.4`, que declara soporte de
Svelte 5 y mantiene Vite 5 (sin más cambios de versión). SvelteKit 2.70
acepta las versiones 3 a 7 del plugin.

## Alternativas consideradas

- **A — Subir ya (elegida).** Cambio de una línea mientras no hay código de UI.
- **B — Esperar a S9.** Sin trabajo hoy, pero la migración llegaría junto con
  la UI y con más código que adaptar.

## Consecuencias

- Desaparece el aviso de peers de Svelte 5 en `npm ci`.
- Verificado: prettier, eslint, svelte-check, vitest y `vite build` en verde;
  `npm ci` funciona con npm 10 (CI) y npm 11.
