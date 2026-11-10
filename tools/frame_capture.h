#pragma once
#include <stdint.h>
#ifdef ALTRUN_CAPTURE_BUILD
#define ALTRUN_CAPTURE_API __declspec(dllexport)
#else
#define ALTRUN_CAPTURE_API __declspec(dllimport)
#endif

struct AltrunCapture;
struct AltrunRegion { int32_t left, top, width, height; };
struct AltrunFrameInfo {
    uint32_t size, version;
    int64_t present_qpc, acquire_begin_qpc, acquire_end_qpc, readback_end_qpc;
    uint32_t accumulated_frames, protected_content;
};
static_assert(sizeof(AltrunFrameInfo) == 48);
extern "C" {
ALTRUN_CAPTURE_API uint32_t altrun_capture_abi_version();
ALTRUN_CAPTURE_API int32_t altrun_capture_open(const AltrunRegion*, AltrunCapture**);
ALTRUN_CAPTURE_API int32_t altrun_capture_poll(AltrunCapture*, uint32_t, uint8_t*, uint64_t, AltrunFrameInfo*);
ALTRUN_CAPTURE_API void altrun_capture_close(AltrunCapture*);
}
