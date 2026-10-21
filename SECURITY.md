# Security Policy

## Supported versions

This is a small hobby project, so only the latest code on `main` (and the newest release, if there is one) gets fixes. Older releases won't be patched.

| Version | Supported |
|---|---|
| `main` / latest release | ✅ |
| anything older | ❌ |

## Reporting a vulnerability

**Please don't open a public issue for security problems.**

Report it privately through GitHub instead: go to the **Security** tab of this repo and click **Report a vulnerability**. Only you and I can see the report.

Include as much as you can:

- what the problem is and what an attacker could do with it
- steps to reproduce, or a proof of concept
- which component (MaxFit Studio, `maxfitctl`, the C++ driver library) and which version or commit

I'll try to reply within a week. This is a side project, so fixes might take a bit longer, but I'll keep you updated and credit you in the fix unless you'd rather stay anonymous.

## What counts

Things I especially want to hear about:

- **Anything that could make the software send a firmware or bootloader command** (`0x0B` / `0x0C`), or otherwise write something to the keyboard that could brick it or leave it in a broken state
- crashes or code execution from a malicious **preset file** (`%APPDATA%\MaxFitStudio\presets\*.json`)
- the driver talking to the **wrong USB device**, or sending commands to anything other than the MAXFIT61's vendor interface
- memory-safety bugs in the C++ driver (buffer overflows when parsing replies from the device, etc.)

## Out of scope

- bugs in Fantech's own firmware or their official app. Report those to Fantech.
- problems that need someone to already have admin access to your PC
- "the lights look wrong" or other normal bugs. Those can go in regular [Issues](../../issues).