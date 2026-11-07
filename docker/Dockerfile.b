# Forge30XX — Perfil B (2x RTX 3060 12 GB). Imagen objetivo <8 GB (RNF-05).
# Idéntico a Dockerfile.a salvo el perfil por defecto; el reparto multi-GPU
# lo resuelve el orquestador en runtime (RF-06b).

# --- Stage: build del orquestador Rust ---
FROM rust:1.80-slim AS build-rust
WORKDIR /src
COPY orchestrator/ orchestrator/
RUN cd orchestrator && cargo build --release --bin forge

# --- Stage: build del frontend ---
FROM node:20-slim AS build-frontend
WORKDIR /src/frontend
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm install
COPY frontend/ ./
RUN npm run build

# --- Stage: runtime ---
FROM nvidia/cuda:12.1.1-runtime-ubuntu22.04 AS runtime
WORKDIR /app
ENV FORGE_PROFILE=B \
    FORGE_UI__PORT=7860
COPY --from=build-rust /src/orchestrator/target/release/forge /usr/local/bin/forge
COPY --from=build-frontend /src/frontend/build /app/frontend
COPY forge.config.toml /app/forge.config.toml
EXPOSE 7860
ENTRYPOINT ["forge"]
CMD ["run", "--ui"]
