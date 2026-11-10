#define ALTRUN_CAPTURE_BUILD
#include "frame_capture_core.hpp"
#include "frame_capture.h"
#include <map>
#include <memory>
#include <mutex>

struct Choice {
    ComPtr<IDXGIAdapter1> adapter;
    ComPtr<IDXGIOutput> output;
    DXGI_OUTPUT_DESC description{};
};
Choice choose_output(const AltrunRegion& region) {
    ComPtr<IDXGIFactory1> factory;
    check(CreateDXGIFactory1(IID_PPV_ARGS(&factory)), "capture_factory");
    for (UINT a = 0; ; ++a) {
        ComPtr<IDXGIAdapter1> adapter;
        const HRESULT ah = factory->EnumAdapters1(a, &adapter);
        if (ah == DXGI_ERROR_NOT_FOUND) break;
        check(ah, "capture_adapter");
        for (UINT o = 0; ; ++o) {
            ComPtr<IDXGIOutput> output;
            const HRESULT oh = adapter->EnumOutputs(o, &output);
            if (oh == DXGI_ERROR_NOT_FOUND) break;
            check(oh, "capture_output");
            DXGI_OUTPUT_DESC description{};
            check(output->GetDesc(&description), "capture_description");
            const RECT r = description.DesktopCoordinates;
            if (description.AttachedToDesktop && r.left <= region.left && r.top <= region.top
                && static_cast<int64_t>(region.left) + region.width <= r.right
                && static_cast<int64_t>(region.top) + region.height <= r.bottom) {
                require(description.Rotation == DXGI_MODE_ROTATION_IDENTITY, "capture_rotation");
                return Choice{adapter, output, description};
            }
        }
    }
    throw Failure{"capture_output_missing", E_INVALIDARG};
}
struct SharedOutput {
    Device gpu;
    ComPtr<IDXGIOutputDuplication> duplication;
    DXGI_OUTPUT_DESC description;
    std::mutex mutex;
    explicit SharedOutput(const Choice& choice) : gpu(choice.adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN), description(choice.description) {
        ComPtr<IDXGIOutput1> output1;
        check(choice.output.As(&output1), "capture_output1");
        check(output1->DuplicateOutput(gpu.device.Get(), &duplication), "capture_duplicate");
    }
};
std::mutex owners_mutex;
std::map<HMONITOR, std::weak_ptr<SharedOutput>> owners;
std::shared_ptr<SharedOutput> get_output(const AltrunRegion& region) {
    const Choice choice = choose_output(region);
    const std::lock_guard<std::mutex> guard(owners_mutex);
    auto& weak = owners[choice.description.Monitor];
    auto owner = weak.lock();
    if (!owner) { owner = std::make_shared<SharedOutput>(choice); weak = owner; }
    return owner;
}
struct AltrunCapture {
    std::shared_ptr<SharedOutput> owner;
    Readback readback;
    UINT left, top;
    explicit AltrunCapture(const AltrunRegion& region) : owner(get_output(region)),
        readback(owner->gpu, static_cast<UINT>(region.width), static_cast<UINT>(region.height)),
        left(static_cast<UINT>(region.left - owner->description.DesktopCoordinates.left)),
        top(static_cast<UINT>(region.top - owner->description.DesktopCoordinates.top)) {}
};
extern "C" {
uint32_t altrun_capture_abi_version() { return 1; }
int32_t altrun_capture_open(const AltrunRegion* region, AltrunCapture** output) {
    if (!output) return E_INVALIDARG;
    *output = nullptr;
    if (!region || region->width <= 0 || region->height <= 0 || region->width > 4096 || region->height > 4096) return E_INVALIDARG;
    try { *output = new AltrunCapture(*region); return S_OK; }
    catch (const Failure& failure) { return failure.hr; }
    catch (...) { return E_FAIL; }
}
int32_t altrun_capture_poll(AltrunCapture* capture, uint32_t timeout, uint8_t* pixels, uint64_t length, AltrunFrameInfo* info) {
    if (!capture || !pixels || !info || info->size != sizeof(*info) || info->version != 1 || timeout > 50) return E_INVALIDARG;
    const uint64_t needed = static_cast<uint64_t>(capture->readback.width) * capture->readback.height * 4;
    if (length < needed) return E_INVALIDARG;
    *info = AltrunFrameInfo{}; info->size = sizeof(*info); info->version = 1;
    try {
        const std::lock_guard<std::mutex> guard(capture->owner->mutex);
        DXGI_OUTDUPL_FRAME_INFO frame{};
        ComPtr<IDXGIResource> resource;
        info->acquire_begin_qpc = now();
        const HRESULT hr = capture->owner->duplication->AcquireNextFrame(timeout, &frame, &resource);
        info->acquire_end_qpc = now();
        if (hr == DXGI_ERROR_WAIT_TIMEOUT) return 1;
        check(hr, "capture_acquire");
        Acquired acquired{capture->owner->duplication.Get()};
        info->present_qpc = frame.LastPresentTime.QuadPart;
        info->accumulated_frames = frame.AccumulatedFrames;
        info->protected_content = frame.ProtectedContentMaskedOut ? 1U : 0U;
        validate_frame_metadata(frame, info->acquire_end_qpc);
        if (!info->present_qpc) { acquired.release(); return 2; }
        require(info->present_qpc > 0 && info->present_qpc <= info->acquire_end_qpc, "capture_timestamp");
        if (info->protected_content) return E_ACCESSDENIED;
        ComPtr<ID3D11Texture2D> texture;
        check(resource.As(&texture), "capture_texture");
        capture->readback.read(texture.Get(), capture->left, capture->top, nullptr, pixels);
        acquired.release();
        info->readback_end_qpc = now();
        return S_OK;
    } catch (const Failure& failure) { return failure.hr; }
    catch (...) { return E_FAIL; }
}
// A caller must not close an owner during a poll. Rust owners are !Send/!Sync.
void altrun_capture_close(AltrunCapture* capture) { delete capture; }
}
