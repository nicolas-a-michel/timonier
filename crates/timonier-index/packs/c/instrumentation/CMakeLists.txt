# Timonier's OpenTelemetry for this project, written by `timonier init` (Timonier's C
# conventions, in `.timonier/conventions/c/`): the C API `timonier_otel`, implemented in C++ on
# opentelemetry-cpp, and the smoke test `timonier_smoke_test`, when opentelemetry-cpp is found
# (vcpkg, Conan, or installed).
find_package(opentelemetry-cpp CONFIG QUIET)
if(opentelemetry-cpp_FOUND)
  # The OTLP/HTTP exporter's target: upstream's and vcpkg's name, or Conan's.
  foreach(candidate otlp_http_exporter exporter_otlp_http)
    if(TARGET opentelemetry-cpp::${candidate})
      set(TIMONIER_OTLP_HTTP opentelemetry-cpp::${candidate})
    endif()
  endforeach()
  enable_language(CXX)
  add_library(timonier_otel STATIC timonier_otel.cpp)
  target_compile_features(timonier_otel PRIVATE cxx_std_17)
  target_include_directories(timonier_otel PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
  target_link_libraries(timonier_otel PRIVATE
    opentelemetry-cpp::trace
    opentelemetry-cpp::resources
    ${TIMONIER_OTLP_HTTP}
    # After the exporter: its static libraries call the SDK's common code (Conan's).
    opentelemetry-cpp::common)
  add_executable(timonier_smoke_test timonier_smoke_test.c)
  target_link_libraries(timonier_smoke_test PRIVATE timonier_otel)
  # A C program linked to C++ code: the C++ runtime links with it.
  set_target_properties(timonier_smoke_test PROPERTIES LINKER_LANGUAGE CXX)
  add_test(NAME timonier_smoke_test COMMAND timonier_smoke_test)
else()
  message(STATUS "timonier: opentelemetry-cpp not found, no export (see .timonier/conventions/c/)")
endif()
