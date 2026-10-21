#include "maxfit/HidTransport.h"

#include <windows.h>
// <windows.h> must precede these.
#include <hidsdi.h>
#include <setupapi.h>

#include <chrono>
#include <thread>
#include <utility>

#pragma comment(lib, "setupapi.lib")
#pragma comment(lib, "hid.lib")

namespace maxfit {
namespace {

std::string lastErrorMessage(const char* what, DWORD err) {
    char buf[256] = {};
    FormatMessageA(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS, nullptr, err,
                   MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), buf, sizeof(buf) - 1, nullptr);
    std::string msg = std::string(what) + " failed (error " + std::to_string(err) + ")";
    if (buf[0] != '\0') {
        std::string detail(buf);
        while (!detail.empty() && (detail.back() == '\r' || detail.back() == '\n' ||
                                   detail.back() == ' ' || detail.back() == '.')) {
            detail.pop_back();
        }
        if (!detail.empty()) msg += ": " + detail;
    }
    return msg;
}

std::string narrow(const wchar_t* w) {
    if (w == nullptr || w[0] == L'\0') return {};
    const int n = WideCharToMultiByte(CP_UTF8, 0, w, -1, nullptr, 0, nullptr, nullptr);
    if (n <= 1) return {};
    std::string out(static_cast<std::size_t>(n - 1), '\0');
    WideCharToMultiByte(CP_UTF8, 0, w, -1, out.data(), n, nullptr, nullptr);
    return out;
}

std::wstring widen(const std::string& s) {
    if (s.empty()) return {};
    const int n = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, nullptr, 0);
    if (n <= 1) return {};
    std::wstring out(static_cast<std::size_t>(n - 1), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, out.data(), n);
    return out;
}

}  // namespace

std::vector<InterfaceInfo> enumerateInterfaces(std::uint16_t vid, std::uint16_t pid) {
    std::vector<InterfaceInfo> result;

    GUID hidGuid{};
    HidD_GetHidGuid(&hidGuid);

    HDEVINFO set = SetupDiGetClassDevsW(&hidGuid, nullptr, nullptr,
                                        DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
    if (set == INVALID_HANDLE_VALUE) return result;

    for (DWORD index = 0;; ++index) {
        SP_DEVICE_INTERFACE_DATA did{};
        did.cbSize = sizeof(did);
        if (!SetupDiEnumDeviceInterfaces(set, nullptr, &hidGuid, index, &did)) break;

        DWORD needed = 0;
        SetupDiGetDeviceInterfaceDetailW(set, &did, nullptr, 0, &needed, nullptr);
        if (needed == 0) continue;

        std::vector<std::uint8_t> buffer(needed);
        auto* detail = reinterpret_cast<PSP_DEVICE_INTERFACE_DETAIL_DATA_W>(buffer.data());
        detail->cbSize = sizeof(SP_DEVICE_INTERFACE_DETAIL_DATA_W);
        if (!SetupDiGetDeviceInterfaceDetailW(set, &did, detail, needed, &needed, nullptr)) {
            continue;
        }

        // Open with no access rights: enough for the descriptor queries below
        // and it never collides with a process holding the device open.
        HANDLE h = CreateFileW(detail->DevicePath, 0, FILE_SHARE_READ | FILE_SHARE_WRITE,
                               nullptr, OPEN_EXISTING, 0, nullptr);
        if (h == INVALID_HANDLE_VALUE) continue;

        HIDD_ATTRIBUTES attrs{};
        attrs.Size = sizeof(attrs);
        if (HidD_GetAttributes(h, &attrs) &&
            (vid == 0 || attrs.VendorID == vid) &&
            (pid == 0 || attrs.ProductID == pid)) {
            InterfaceInfo info;
            info.path = narrow(detail->DevicePath);
            info.vendorId = attrs.VendorID;
            info.productId = attrs.ProductID;

            wchar_t text[256];
            if (HidD_GetProductString(h, text, sizeof(text))) info.product = narrow(text);
            if (HidD_GetManufacturerString(h, text, sizeof(text))) info.manufacturer = narrow(text);

            PHIDP_PREPARSED_DATA preparsed = nullptr;
            if (HidD_GetPreparsedData(h, &preparsed)) {
                HIDP_CAPS caps{};
                if (HidP_GetCaps(preparsed, &caps) == HIDP_STATUS_SUCCESS) {
                    info.usagePage = caps.UsagePage;
                    info.usage = caps.Usage;
                    info.inputLength = caps.InputReportByteLength;
                    info.outputLength = caps.OutputReportByteLength;
                }
                HidD_FreePreparsedData(preparsed);
            }
            result.push_back(std::move(info));
        }
        CloseHandle(h);
    }

    SetupDiDestroyDeviceInfoList(set);
    return result;
}

bool findControlInterface(InterfaceInfo& out, std::uint16_t vid, std::uint16_t pid) {
    for (auto& iface : enumerateInterfaces(vid, pid)) {
        if (iface.usagePage == kUsagePage && iface.usage == kUsage &&
            iface.outputLength == kReportSize) {
            out = std::move(iface);
            return true;
        }
    }
    return false;
}

HidTransport::~HidTransport() { close(); }

HidTransport::HidTransport(HidTransport&& other) noexcept
    : handle_(other.handle_),
      event_(other.event_),
      readOnly_(other.readOnly_),
      path_(std::move(other.path_)) {
    other.handle_ = nullptr;
    other.event_ = nullptr;
}

HidTransport& HidTransport::operator=(HidTransport&& other) noexcept {
    if (this != &other) {
        close();
        handle_ = other.handle_;
        event_ = other.event_;
        readOnly_ = other.readOnly_;
        path_ = std::move(other.path_);
        other.handle_ = nullptr;
        other.event_ = nullptr;
    }
    return *this;
}

void HidTransport::open(const std::string& devicePath) {
    close();
    const std::wstring wpath = widen(devicePath);

    HANDLE h = CreateFileW(wpath.c_str(), GENERIC_READ | GENERIC_WRITE,
                           FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING,
                           FILE_FLAG_OVERLAPPED, nullptr);
    bool ro = false;
    if (h == INVALID_HANDLE_VALUE) {
        h = CreateFileW(wpath.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE,
                        nullptr, OPEN_EXISTING, FILE_FLAG_OVERLAPPED, nullptr);
        ro = true;
    }
    if (h == INVALID_HANDLE_VALUE) {
        throw Error(lastErrorMessage("opening HID device", GetLastError()));
    }

    HANDLE ev = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    if (ev == nullptr) {
        const DWORD err = GetLastError();
        CloseHandle(h);
        throw Error(lastErrorMessage("creating overlapped event", err));
    }

    handle_ = h;
    event_ = ev;
    readOnly_ = ro;
    path_ = devicePath;

    // Let the driver buffer a few reports so a reply is not dropped while we
    // are still finishing the write.
    HidD_SetNumInputBuffers(h, 64);
}

void HidTransport::close() {
    if (handle_ != nullptr) {
        CancelIo(handle_);
        CloseHandle(handle_);
        handle_ = nullptr;
    }
    if (event_ != nullptr) {
        CloseHandle(event_);
        event_ = nullptr;
    }
    readOnly_ = false;
    path_.clear();
}

void HidTransport::write(const Report& r) {
    if (handle_ == nullptr) throw Error("write on a closed device");
    if (readOnly_) {
        throw Error("device is open read-only; another process may be holding it");
    }

    OVERLAPPED ov{};
    ov.hEvent = event_;
    ResetEvent(event_);

    DWORD written = 0;
    if (WriteFile(handle_, r.data(), static_cast<DWORD>(r.size()), &written, &ov)) return;

    const DWORD err = GetLastError();
    if (err != ERROR_IO_PENDING) {
        // Some firmware prefers the HID-class setter over a raw write.
        if (HidD_SetOutputReport(handle_, const_cast<std::uint8_t*>(r.data()),
                                 static_cast<ULONG>(r.size()))) {
            return;
        }
        throw Error(lastErrorMessage("writing HID report", err));
    }

    if (WaitForSingleObject(event_, 1000) != WAIT_OBJECT_0) {
        CancelIo(handle_);
        throw Error("timed out writing HID report");
    }
    if (!GetOverlappedResult(handle_, &ov, &written, FALSE)) {
        throw Error(lastErrorMessage("writing HID report", GetLastError()));
    }
}

bool HidTransport::read(Report& out, unsigned timeoutMs) {
    if (handle_ == nullptr) throw Error("read on a closed device");

    out.fill(0);
    OVERLAPPED ov{};
    ov.hEvent = event_;
    ResetEvent(event_);

    DWORD got = 0;
    if (ReadFile(handle_, out.data(), static_cast<DWORD>(out.size()), &got, &ov)) return true;

    const DWORD err = GetLastError();
    if (err != ERROR_IO_PENDING) {
        throw Error(lastErrorMessage("reading HID report", err));
    }
    if (WaitForSingleObject(event_, timeoutMs) != WAIT_OBJECT_0) {
        CancelIo(handle_);
        return false;
    }
    return GetOverlappedResult(handle_, &ov, &got, FALSE) != 0;
}

void HidTransport::drain(unsigned quietMs) {
    if (handle_ == nullptr) return;
    HidD_FlushQueue(handle_);
    Report scratch{};
    while (read(scratch, quietMs)) {
        // Discard.
    }
}

bool HidTransport::request(const Report& req, Command cmd, Report& reply, unsigned timeoutMs) {
    drain();
    write(req);

    const auto deadline =
        std::chrono::steady_clock::now() + std::chrono::milliseconds(timeoutMs);
    while (std::chrono::steady_clock::now() < deadline) {
        const auto remaining = std::chrono::duration_cast<std::chrono::milliseconds>(
                                   deadline - std::chrono::steady_clock::now())
                                   .count();
        if (remaining <= 0) break;
        if (!read(reply, static_cast<unsigned>(remaining))) break;
        if (isReplyTo(reply, cmd)) return true;
        // Not ours (the firmware emits extra pages for some queries); keep reading.
    }
    return false;
}

}  // namespace maxfit
