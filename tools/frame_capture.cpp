#define ALTRUN_CAPTURE_BUILD
#include "frame_capture.h"
#include <windows.h>

extern "C" {
uint32_t altrun_capture_abi_version() { return 0; }
int32_t altrun_capture_open(const AltrunRegion*, AltrunCapture**) { return E_NOTIMPL; }
int32_t altrun_capture_poll(AltrunCapture*, uint32_t, uint8_t*, uint64_t, AltrunFrameInfo*) { return E_NOTIMPL; }
void altrun_capture_close(AltrunCapture*) {}
}
