// Test-only ABI fixture. It never opens a display, device or user window.
#define ALTRUN_CAPTURE_BUILD
#include "../../tools/frame_capture.h"
#include <windows.h>
#include <new>

struct AltrunCapture { int32_t width, height; unsigned calls = 0; };
unsigned closed = 0;
extern "C" {
uint32_t altrun_capture_abi_version() {
#ifdef ALTRUN_BAD_ABI
    return 0;
#else
    return 1;
#endif
}
int32_t altrun_capture_open(const AltrunRegion* region, AltrunCapture** output) {
    if (!output) return E_INVALIDARG;
    *output = nullptr;
    if (!region || region->width <= 0 || region->height <= 0 || region->width > 4096 || region->height > 4096) return E_INVALIDARG;
    *output = new (std::nothrow) AltrunCapture{region->width, region->height};
    return *output ? S_OK : E_OUTOFMEMORY;
}
int32_t altrun_capture_poll(AltrunCapture* capture, uint32_t timeout, uint8_t* pixels, uint64_t length, AltrunFrameInfo* info) {
    if (!capture || !pixels || !info || timeout > 50 || length < static_cast<uint64_t>(capture->width) * capture->height * 4) return E_INVALIDARG;
#ifdef ALTRUN_POISON_POINTER
    *info = AltrunFrameInfo{48, ALTRUN_POISON_POINTER == 3 ? 99U : 1U, 0, 19'000, 20'000, 0,
        ALTRUN_POISON_POINTER == 2 ? 3U : 0U, (ALTRUN_POISON_POINTER == 1 || ALTRUN_POISON_POINTER == 4) ? 1U : 0U};
    return ALTRUN_POISON_POINTER == 4 ? 1 : 2;
#else
    if (capture->calls++ == 0) return 1;
    if (capture->calls == 2) return 2;
    *info = AltrunFrameInfo{48, 1, 20'000, 19'000, 21'932, 71'495, 1, 0};
    for (int32_t y = 0; y < capture->height; ++y) for (int32_t x = 0; x < capture->width; ++x) {
        const uint64_t offset = (static_cast<uint64_t>(y) * capture->width + x) * 4;
        pixels[offset] = static_cast<uint8_t>(x);
        pixels[offset + 1] = static_cast<uint8_t>(y);
        pixels[offset + 2] = 127;
        pixels[offset + 3] = 255;
    }
    return S_OK;
#endif
}
void altrun_capture_close(AltrunCapture* capture) { if (capture) { ++closed; delete capture; } }
__declspec(dllexport) uint32_t altrun_fixture_closed() { return closed; }
}
