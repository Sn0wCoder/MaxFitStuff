// HidTransport.h -- minimal Win32 HID transport for fixed 64-byte reports.
//
// Deliberately dependency-free: setupapi + hid.dll only, no hidapi. Reads use
// overlapped I/O so a missing reply times out instead of hanging the caller.
#pragma once

#include <cstdint>
#include <stdexcept>
#include <string>
#include <vector>

#include "Protocol.h"

namespace maxfit {

class Error : public std::runtime_error {
public:
    explicit Error(const std::string& what) : std::runtime_error(what) {}
};

// One HID interface exposed by a USB device.
struct InterfaceInfo {
    std::string path;
    std::uint16_t vendorId = 0;
    std::uint16_t productId = 0;
    std::uint16_t usagePage = 0;
    std::uint16_t usage = 0;
    std::uint16_t inputLength = 0;
    std::uint16_t outputLength = 0;
    std::string product;
    std::string manufacturer;
};

// Every HID interface matching vid/pid (0 = wildcard).
std::vector<InterfaceInfo> enumerateInterfaces(std::uint16_t vid = kVendorId,
                                               std::uint16_t pid = kProductId);

// The vendor control interface: usage page 0xFF1B, usage 0x91, 64-byte reports.
// Returns false when the keyboard is not plugged in.
bool findControlInterface(InterfaceInfo& out,
                          std::uint16_t vid = kVendorId,
                          std::uint16_t pid = kProductId);

class HidTransport {
public:
    HidTransport() = default;
    ~HidTransport();

    HidTransport(const HidTransport&) = delete;
    HidTransport& operator=(const HidTransport&) = delete;
    HidTransport(HidTransport&& other) noexcept;
    HidTransport& operator=(HidTransport&& other) noexcept;

    // Throws Error on failure.
    void open(const std::string& devicePath);
    void close();
    bool isOpen() const { return handle_ != nullptr; }

    // True when the device could only be opened for reading (another process
    // holds it exclusively, or the account lacks write access).
    bool readOnly() const { return readOnly_; }

    const std::string& path() const { return path_; }

    // Writes exactly kReportSize bytes. Throws Error on failure.
    void write(const Report& r);

    // Reads one report. Returns false on timeout. Throws Error on I/O failure.
    bool read(Report& out, unsigned timeoutMs = 400);

    // Drops any reports the device has already queued, so a following
    // request/reply exchange cannot pick up a stale answer.
    void drain(unsigned quietMs = 40);

    // write() then read() until a reply carrying `cmd` arrives, or timeout.
    bool request(const Report& req, Command cmd, Report& reply, unsigned timeoutMs = 600);

private:
    void* handle_ = nullptr;  // HANDLE
    void* event_ = nullptr;   // HANDLE, for overlapped reads
    bool readOnly_ = false;
    std::string path_;
};

}  // namespace maxfit
