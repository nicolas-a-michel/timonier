# Timonier's OpenTelemetry for this project, written by `timonier init` (Timonier's C++
# conventions, in `.timonier/conventions/cpp/`): the starter `timonier_otel` and the smoke test
# `timonier_smoke_test`, when opentelemetry-cpp is found (vcpkg, Conan, or installed).
find_package(opentelemetry-cpp CONFIG QUIET)
if(opentelemetry-cpp_FOUND)
  # The OTLP/HTTP exporter's target: upstream's and vcpkg's name, or Conan's.
  foreach(candidate otlp_http_exporter exporter_otlp_http)
    if(TARGET opentelemetry-cpp::${candidate})
      set(TIMONIER_OTLP_HTTP opentelemetry-cpp::${candidate})
    endif()
  endforeach()
  add_library(timonier_otel INTERFACE)
  target_include_directories(timonier_otel INTERFACE ${CMAKE_CURRENT_SOURCE_DIR})
  target_link_libraries(timonier_otel INTERFACE
    opentelemetry-cpp::trace
    opentelemetry-cpp::resources
    ${TIMONIER_OTLP_HTTP}
    # After the exporter: its static libraries call the SDK's common code (Conan's).
    opentelemetry-cpp::common)
  add_executable(timonier_smoke_test timonier_smoke_test.cpp)
  target_compile_features(timonier_smoke_test PRIVATE cxx_std_17)
  target_link_libraries(timonier_smoke_test PRIVATE timonier_otel)
  add_test(NAME timonier_smoke_test COMMAND timonier_smoke_test)
else()
  message(STATUS "timonier: opentelemetry-cpp not found, no export (see .timonier/conventions/cpp/)")
endif()
