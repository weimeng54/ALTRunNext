#include "frame_capture_core.hpp"

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
