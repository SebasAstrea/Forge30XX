# Forge30XX — capa de inferencia (C++/CUDA).
# Motor: fork TheTom/llama-cpp-turboquant servido por HTTP (ADR-0004). El fork
# se compila fuera del repo (docs/inference.md); este proyecto es un esqueleto.

cmake_minimum_required(VERSION 3.28)
project(forge_inference LANGUAGES CXX)

option(GGML_CUDA "Compilar con soporte CUDA" ON)

# Objetivo vacío para validar la configuración CMake.
add_library(forge_inference INTERFACE)
target_compile_features(forge_inference INTERFACE cxx_std_17)

message(STATUS "forge_inference: esqueleto. GGML_CUDA=${GGML_CUDA}")
