#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include "frame_capture.h"

template<class Fn> Fn symbol(HMODULE module, const char* name) {
    const FARPROC address = GetProcAddress(module, name);
    if (!address) throw name;
    Fn function{};
    static_assert(sizeof(function) == sizeof(address));
    std::memcpy(&function, &address, sizeof(function));
    return function;
}
void expect(bool value, const char* name) { if (!value) throw name; }
int main() {
    HMODULE library = nullptr;
    try {
        wchar_t exe[32768]{};
        const DWORD length = GetModuleFileNameW(nullptr, exe, 32768);
        expect(length > 0 && length < 32768, "executable_path");
        const auto path = std::filesystem::path(exe).parent_path() / "altrun-frame-capture.dll";
        library = LoadLibraryExW(path.c_str(), nullptr, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
        expect(library != nullptr, "load_exact_adjacent_library");
        const auto version = symbol<decltype(&altrun_capture_abi_version)>(library, "altrun_capture_abi_version");
        const auto open = symbol<decltype(&altrun_capture_open)>(library, "altrun_capture_open");
        const auto poll = symbol<decltype(&altrun_capture_poll)>(library, "altrun_capture_poll");
        const auto close = symbol<decltype(&altrun_capture_close)>(library, "altrun_capture_close");
        expect(version() == 1, "supported_abi_version");
        AltrunCapture* capture = reinterpret_cast<AltrunCapture*>(static_cast<uintptr_t>(1));
        const AltrunRegion invalid{400, 329, 0, 459};
        expect(open(&invalid, &capture) == E_INVALIDARG && capture == nullptr, "invalid_region_is_not_an_owner");
        expect(open(nullptr, &capture) == E_INVALIDARG && capture == nullptr, "null_region");
        expect(open(&invalid, nullptr) == E_INVALIDARG, "null_owner_output");
        uint8_t sentinel = 77;
        AltrunFrameInfo info{}; info.size = sizeof(info); info.version = 1;
        expect(poll(nullptr, 0, &sentinel, 1, &info) == E_INVALIDARG && sentinel == 77, "null_poll_does_not_touch_pixels");
        close(nullptr);
        FreeLibrary(library);
        std::puts("Capture ABI boundary self-test passed");
        return 0;
    } catch (const char* failed) {
        if (library) FreeLibrary(library);
        std::fprintf(stderr, "Capture ABI self-test failed: %s\n", failed);
        return 2;
    } catch (...) {
        if (library) FreeLibrary(library);
        return 2;
    }
}
