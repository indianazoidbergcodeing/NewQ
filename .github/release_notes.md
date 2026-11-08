## Downloads

Everything NewQ needs is in the download, including FFmpeg and the MediaMTX
streaming server.

| Platform | File |
|----------|------|
| Windows 10/11 (x64) | `NewQ-{{TAG}}-windows-x64.zip` |
| Linux (x64, glibc 2.35+) | `NewQ-{{TAG}}-linux-x64.tar.gz` |
| macOS 11+ (Apple Silicon and Intel) | `NewQ-{{TAG}}-macos-universal.zip` |

**Windows:** unzip and run `newq.exe`. These builds aren't code-signed yet, so
SmartScreen may warn: choose *More info → Run anyway*.

**Linux:** extract and run `./newq`. Needs ALSA (`libasound2`) and a
Vulkan- or OpenGL-capable graphics driver.

**macOS:** unzip and move `NewQ.app` to Applications. Builds from this fork are
only ad-hoc signed, not notarised by Apple, so the first time macOS will refuse
to open it: right-click the app and choose *Open*, then *Open* again.

`SHA256SUMS.txt` lists checksums for every file.

The bundled FFmpeg is an unmodified LGPL build: from
[BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) on Windows and
Linux, and compiled from the official FFmpeg release source by
`scripts/build_ffmpeg_macos.sh` on macOS. See `SOURCE.txt` in the download's
`ffmpeg`/`FFmpeg` folder for the exact version and source links.

The bundled MediaMTX is an unmodified build from
[bluenviron/mediamtx](https://github.com/bluenviron/mediamtx) (MIT licence),
pinned by checksum in `scripts/mediamtx_pins.txt`; see the `mediamtx` folder in
the download for its licence and source link.

## What's new in this fork

- **Streaming server in NewQ:** in a display's *Send out* section, *Streaming
  server (MediaMTX)* can now run the server itself (*Run the server in NewQ*).
- **Player link and QR code:** show the web player link and a QR code so phones
  open the stream without typing the address.
- **Network adapter:** choose which network connection the server listens on and
  the links carry.
- **Test server:** checks the server's ports.
