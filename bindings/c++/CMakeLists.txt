cmake_minimum_required(VERSION 3.16)
project(eiffel VERSION 0.1)

set(CMAKE_CXX_STANDARD 20)
# add_compile_options(-stdlib=libstdc++)
#add_compile_options(-Wall)

# When building cmake, make sure to define command line argument -DCMAKE_TOOLCHAIN_FILE=./vcpkg/scripts/buildsystems/vcpkg.cmake
# more details: https://github.com/microsoft/vcpkg#using-vcpkg-with-cmake

# https://rymc.io/blog/2019/rust-cargo-lcrypto-openssl-mac-and-you/

set(OPENSSL_ROOT_DIR "/usr/local/opt/openssl@3")
set(CMAKE_BUILD_TYPE Release)

find_package(OpenSSL REQUIRED)
find_package(emp-tool REQUIRED)
find_package(emp-ot REQUIRED)

include_directories(${OPENSSL_INCLUDE_DIR})
include_directories(${EMP-TOOL_INCLUDE_DIRS})
include_directories(${EMP-OT_INCLUDE_DIRS})

include(emp-base.cmake)

# common
add_library(eiffelcpp STATIC
interface.cpp
demo.cpp
)

target_link_libraries(eiffelcpp ${OPENSSL_LIBRARIES})

#set_source_files_properties(interface.h LANGUAGE c)

install(TARGETS eiffelcpp DESTINATION .)



