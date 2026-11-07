# Forge30XX — capa de inferencia (C++/CUDA).
# Integra el fork llama-cpp-turboquant (TurboQuant + KVFlash). docs/SDD.md sección 8.
# La integración real (gRPC a llama-server, kernels) se aborda en Sprint S1-S3.

cmake_minimum_required(VERSION 3.28)
project(forge_inference LANGUAGES CXX)

option(GGML_CUDA "Compilar con soporte CUDA" ON)

# El fork se añadirá como submódulo en vendor/llama-cpp-turboquant.
# set(LLAMA_CPP_DIR "${CMAKE_CURRENT_SOURCE_DIR}/vendor/llama-cpp-turboquant")
# add_subdirectory(${LLAMA_CPP_DIR})

# Placeholder de bootstrap: objetivo vacío para validar la configuración CMake.
add_library(forge_inference INTERFACE)
target_compile_features(forge_inference INTERFACE cxx_std_17)

message(STATUS "forge_inference: esqueleto de bootstrap. GGML_CUDA=${GGML_CUDA}")
message(STATUS "Integración de llama-cpp-turboquant pendiente (Sprint S1-S3).")
