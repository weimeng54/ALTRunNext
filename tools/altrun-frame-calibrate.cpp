// Experimental observer only. No input, visible window or launcher access.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>
#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "dxgi.lib")
#pragma comment(lib, "user32.lib")
using Microsoft::WRL::ComPtr;
namespace fs = std::filesystem;
struct Failure { const char* stage; HRESULT hr; };
void require(bool ok, const char* stage) { if (!ok) throw Failure{stage, E_FAIL}; }
void check(HRESULT hr, const char* stage) { if (FAILED(hr)) throw Failure{stage, hr}; }
int64_t now() { LARGE_INTEGER t{}; require(QueryPerformanceCounter(&t) != 0, "qpc"); return t.QuadPart; }
volatile uint64_t hash_sink = 0;
struct Report {
    fs::path path;
    const char* stage = "setup";
    HRESULT error = S_OK;
    bool complete = false, synthetic = false, desktop = false;
    int64_t frequency = 0;
    unsigned timeouts = 0, pointer_only = 0;
    std::vector<std::string> rows;
    bool save() const {
        std::ofstream out(path, std::ios::binary);
        if (!out) return false;
        out << "{\"measurement_kind\":\"desktop-frame-calibration\",\"decision\":\"pending\",\"content_verified\":false,\"session_status\":\""
            << (complete ? "observed" : "rejected") << "\",\"stage\":\"" << stage << "\",\"hresult\":" << static_cast<int64_t>(error)
            << ",\"synthetic_gpu_verified\":" << (synthetic ? "true" : "false") << ",\"desktop_sampled\":" << (desktop ? "true" : "false")
            << ",\"qpc_frequency\":" << frequency << ",\"timeouts\":" << timeouts << ",\"pointer_only\":" << pointer_only << ",\"samples\":[],\"frames\":[";
        for (size_t i = 0; i < rows.size(); ++i) { if (i) out << ','; out << rows[i]; }
        out << "],\"limitations\":[\"Desktop update timestamp is not physical presentation\",\"No independent glyph oracle\",\"No launcher parity evidence\",\"No desktop pixels or hashes saved\"]}";
        out.close(); return !out.fail();
    }
};
struct Device {
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> context;
    Device(IDXGIAdapter* adapter, D3D_DRIVER_TYPE type) {
        D3D_FEATURE_LEVEL level{};
        check(D3D11CreateDevice(adapter, type, nullptr, D3D11_CREATE_DEVICE_BGRA_SUPPORT, nullptr, 0, D3D11_SDK_VERSION, &device, &level, &context), "create_device");
    }
};
struct Readback {
    Device& gpu;
    UINT width, height;
    ComPtr<ID3D11Texture2D> staging;
    int64_t copy_end = 0, map_end = 0, hash_end = 0;
    Readback(Device& device, UINT w, UINT h) : gpu(device), width(w), height(h) {
        D3D11_TEXTURE2D_DESC desc{};
        desc.Width = width; desc.Height = height; desc.MipLevels = 1; desc.ArraySize = 1;
        desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM; desc.SampleDesc.Count = 1;
        desc.Usage = D3D11_USAGE_STAGING; desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        check(gpu.device->CreateTexture2D(&desc, nullptr, &staging), "create_staging");
    }
    void read(ID3D11Texture2D* source, UINT x, UINT y, const std::vector<uint8_t>* expected = nullptr) {
        D3D11_TEXTURE2D_DESC desc{}; source->GetDesc(&desc);
        require(desc.Format == DXGI_FORMAT_B8G8R8A8_UNORM && desc.SampleDesc.Count == 1
            && desc.Width >= width && desc.Height >= height && x <= desc.Width - width && y <= desc.Height - height, "source_region");
        D3D11_BOX box{x, y, 0, x + width, y + height, 1};
        gpu.context->CopySubresourceRegion(staging.Get(), 0, 0, 0, 0, source, 0, &box);
        copy_end = now();
        D3D11_MAPPED_SUBRESOURCE mapped{};
        check(gpu.context->Map(staging.Get(), 0, D3D11_MAP_READ, 0, &mapped), "map_readback");
        struct Unmap { ID3D11DeviceContext* context; ID3D11Resource* resource; ~Unmap() { context->Unmap(resource, 0); } } unmap{gpu.context.Get(), staging.Get()};
        map_end = now();
        require(mapped.pData && mapped.RowPitch >= width * 4, "row_pitch");
        uint64_t hash = UINT64_C(0xcbf29ce484222325);
        const auto* bytes = static_cast<const uint8_t*>(mapped.pData);
        for (UINT row = 0; row < height; ++row) {
            const auto* current = bytes + static_cast<size_t>(row) * mapped.RowPitch;
            for (UINT column = 0; column < width * 4; ++column) {
                if (expected) require(current[column] == (*expected)[static_cast<size_t>(row) * width * 4 + column], "controlled_gpu_pattern");
                hash = (hash ^ current[column]) * UINT64_C(0x100000001b3);
            }
        }
        hash_sink = hash; hash_end = now();
    }
};
void self_test() {
    Device gpu(nullptr, D3D_DRIVER_TYPE_WARP);
    Readback reader(gpu, 37, 19);
    for (unsigned iteration = 0; iteration < 8; ++iteration) {
        std::vector<uint8_t> pixels(41 * 29 * 4), expected(37 * 19 * 4);
        for (UINT y = 0; y < 29; ++y) for (UINT x = 0; x < 41; ++x) {
            const size_t offset = (static_cast<size_t>(y) * 41 + x) * 4;
            pixels[offset] = static_cast<uint8_t>(x + iteration * 13);
            pixels[offset + 1] = static_cast<uint8_t>(y + iteration * 7);
            pixels[offset + 2] = static_cast<uint8_t>(x + y + iteration * 19);
            pixels[offset + 3] = 255;
        }
        for (UINT y = 0; y < 19; ++y) for (UINT x = 0; x < 37; ++x) for (UINT c = 0; c < 4; ++c)
            expected[(static_cast<size_t>(y) * 37 + x) * 4 + c] = pixels[(static_cast<size_t>(y + 3) * 41 + x + 2) * 4 + c];
        D3D11_TEXTURE2D_DESC desc{};
        desc.Width = 41; desc.Height = 29; desc.MipLevels = 1; desc.ArraySize = 1;
        desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM; desc.SampleDesc.Count = 1; desc.Usage = D3D11_USAGE_IMMUTABLE;
        D3D11_SUBRESOURCE_DATA data{}; data.pSysMem = pixels.data(); data.SysMemPitch = 41 * 4;
        ComPtr<ID3D11Texture2D> texture;
        check(gpu.device->CreateTexture2D(&desc, &data, &texture), "create_controlled_texture");
        reader.read(texture.Get(), 2, 3, &expected);
    }
}
struct Acquired {
    IDXGIOutputDuplication* duplication;
    bool held = true;
    void release() { held = false; check(duplication->ReleaseFrame(), "release_frame"); }
    ~Acquired() { if (held) duplication->ReleaseFrame(); }
};
void sample_desktop(Report& report) {
    require(SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) != 0, "dpi_context");
    ComPtr<IDXGIFactory1> factory; check(CreateDXGIFactory1(IID_PPV_ARGS(&factory)), "create_factory");
    ComPtr<IDXGIAdapter1> selected_adapter;
    ComPtr<IDXGIOutput> selected_output;
    DXGI_OUTPUT_DESC output_desc{};
    for (UINT a = 0; !selected_output; ++a) {
        ComPtr<IDXGIAdapter1> adapter;
        const HRESULT adapter_hr = factory->EnumAdapters1(a, &adapter);
        if (adapter_hr == DXGI_ERROR_NOT_FOUND) break;
        check(adapter_hr, "enumerate_adapters");
        for (UINT o = 0; ; ++o) {
            ComPtr<IDXGIOutput> output;
            const HRESULT output_hr = adapter->EnumOutputs(o, &output);
            if (output_hr == DXGI_ERROR_NOT_FOUND) break;
            check(output_hr, "enumerate_outputs");
            DXGI_OUTPUT_DESC desc{}; check(output->GetDesc(&desc), "output_description");
            const RECT r = desc.DesktopCoordinates;
            if (desc.AttachedToDesktop && r.left <= 400 && r.top <= 329 && r.right >= 1240 && r.bottom >= 788) {
                selected_adapter = adapter; selected_output = output; output_desc = desc; break;
            }
        }
    }
    require(selected_output.Get() != nullptr, "region_requires_single_output");
    require(output_desc.Rotation == DXGI_MODE_ROTATION_IDENTITY, "rotated_output_unsupported");
    Device gpu(selected_adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN);
    ComPtr<IDXGIOutput1> output1; check(selected_output.As(&output1), "query_output1");
    ComPtr<IDXGIOutputDuplication> duplication;
    check(output1->DuplicateOutput(gpu.device.Get(), &duplication), "duplicate_output");
    Readback reader(gpu, 840, 459);
    const UINT left = static_cast<UINT>(400 - output_desc.DesktopCoordinates.left);
    const UINT top = static_cast<UINT>(329 - output_desc.DesktopCoordinates.top);
    report.desktop = true;
    const int64_t start = now();
    while (now() - start < report.frequency * 8 && report.rows.size() < 200) {
        DXGI_OUTDUPL_FRAME_INFO info{};
        ComPtr<IDXGIResource> resource;
        const int64_t acquire_begin = now();
        const HRESULT hr = duplication->AcquireNextFrame(50, &info, &resource);
        const int64_t acquire_end = now();
        if (hr == DXGI_ERROR_WAIT_TIMEOUT) { ++report.timeouts; continue; }
        check(hr, "acquire_frame");
        Acquired acquired{duplication.Get()};
        if (!info.LastPresentTime.QuadPart) { ++report.pointer_only; acquired.release(); continue; }
        require(info.LastPresentTime.QuadPart > 0 && info.LastPresentTime.QuadPart <= acquire_end, "invalid_frame_timestamp");
        require(!info.ProtectedContentMaskedOut, "protected_content_masked");
        ComPtr<ID3D11Texture2D> texture; check(resource.As(&texture), "query_frame_texture");
        const int64_t copy_begin = now();
        reader.read(texture.Get(), left, top);
        acquired.release();
        const int64_t release_end = now();
        std::ostringstream row;
        row << "{\"acquire_begin_qpc\":" << acquire_begin << ",\"acquire_end_qpc\":" << acquire_end
            << ",\"copy_begin_qpc\":" << copy_begin << ",\"copy_end_qpc\":" << reader.copy_end
            << ",\"map_end_qpc\":" << reader.map_end << ",\"hash_end_qpc\":" << reader.hash_end
            << ",\"release_end_qpc\":" << release_end << ",\"last_present_qpc\":" << info.LastPresentTime.QuadPart
            << ",\"accumulated_frames\":" << info.AccumulatedFrames << ",\"rects_coalesced\":" << (info.RectsCoalesced ? "true" : "false")
            << ",\"protected_content_masked\":false}";
        report.rows.push_back(row.str());
    }
}
void validate_path(const fs::path& path) {
    const auto text = path.wstring();
    require(path.is_absolute() && text.size() > 3 && (text[0] == L'D' || text[0] == L'd') && text[1] == L':' && text[2] == L'\\', "output_requires_d_drive");
}
int main(int argc, char** argv) {
    try {
        fs::path output;
        bool synthetic_only = false;
        if (argc == 4 && std::string(argv[1]) == "--self-test" && std::string(argv[2]) == "--output") {
            synthetic_only = true; output = fs::path(argv[3]);
        } else {
            require(argc == 1, "invalid_arguments");
            std::vector<wchar_t> exe(32768);
            const DWORD length = GetModuleFileNameW(nullptr, exe.data(), static_cast<DWORD>(exe.size()));
            require(length > 0 && length < exe.size(), "executable_path");
            const fs::path folder = fs::path(exe.data()).parent_path();
            std::ifstream profile(folder / "frame-calibration.profile", std::ios::binary);
            const std::string text((std::istreambuf_iterator<char>(profile)), std::istreambuf_iterator<char>());
            require(text == "ALTRUN-FRAME-CALIBRATION\n", "explicit_profile_required");
            output = folder / "frame-calibration-result.json";
        }
        validate_path(output); fs::create_directories(output.parent_path());
        Report report; report.path = output;
        try {
            LARGE_INTEGER frequency{}; require(QueryPerformanceFrequency(&frequency) && frequency.QuadPart > 0, "qpc_frequency");
            report.frequency = frequency.QuadPart;
            report.stage = "synthetic_gpu_readback";
            self_test(); report.synthetic = true;
            if (!synthetic_only) { report.stage = "desktop_sampling"; sample_desktop(report); }
            report.stage = "complete"; report.complete = true;
        } catch (const Failure& failure) { report.stage = failure.stage; report.error = failure.hr; }
        require(report.save(), "save_report");
        return report.complete ? 0 : 2;
    } catch (const Failure& failure) {
        std::fprintf(stderr, "Frame experiment rejected: %s (HRESULT %ld)\n", failure.stage, static_cast<long>(failure.hr));
        return 2;
    } catch (...) { std::fputs("Frame experiment failed\n", stderr); return 2; }
}
