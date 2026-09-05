# Native EtherCAT backend

SOEM is vendored from https://github.com/OpenEtherCATsociety/SOEM, tag v1.4.0,
commit `abbf0d42e38d6cfbaa4c1e9e8e07ace651c386fd`. Only the core and Windows source/header
subset is included. The upstream license and all source notices are retained.
The application remains GPL-3.0-only; SOEM has its own GPLv2 license with a linking
exception. See `soem/LICENSE`, including its EtherCAT master licensing notice.

This product includes software developed by the Computer Systems Engineering Group
at Lawrence Berkeley Laboratory. WinPcap SDK header notices are retained in
`soem/oshw/win32/wpcap/Include` and included with the installer. No WinPcap/Npcap
driver, DLL or import library is redistributed.

`build.rs` compiles C sources using the existing MSVC toolchain and `cc`.
`bridge.c` provides a fixed-width C ABI and loads Npcap from the Windows system
Npcap directory, using restricted DLL search flags. The application can start and
use offline recording/decoding and Modbus without Npcap. Installing Npcap requires
an application restart. Live capture and master access are Windows-only.

All master operations, including close and error-list access, run under one Rust
mutex. Only one SOEM default context exists. Passive capture owns a separate
handle in its worker and does not access SOEM globals. Both sources expose their
capture boundary and driver drop counters; a missing counter means unknown.

Local upstream patches (kept small and documented for future upgrades):

- `ethercatconfiglist.h`: remove the deprecated built-in device table; keep only
  sentinels. Discovery uses each device's SII, not vendor-specific presets.
- `nicdrv.c`: release critical sections on failed open; check actual captured
  lengths and response index before copying into SOEM buffers.
- `ethercatcoe.c`: validate initial upload mailbox length, command/subindex,
  nonnegative advertised size; bound segmented uploads, validate toggle and final
  length, terminate on mailbox failure and on a two-second transfer deadline.
- The application pcap shim validates Ethernet/EtherCAT boundaries before passing
  frames into SOEM, after giving the recorder the original observation.

The master initializes mailbox communication in PRE-OP. It never maps PDO outputs,
requests OP, enables a drive, or runs a motion loop. Configuration writes use normal
CoE SDO access with write-before comparison, explicit confirmation and readback.
SOEM itself performs protocol-level frame/mailbox retries; the application does not
repeat a failed SDO write. Hardware timing and device compatibility require testing
on the intended isolated bus.
