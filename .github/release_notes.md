## What's new in 0.20.1.1

Built on NewQ 0.20.1. Choose how a stream to the server is encoded:

- **Video codec**: H.264, H.265, AV1 or VP9, with a note on which protocols and
  viewers carry each (SRT cannot carry AV1 or VP9).
- **Encoder list**: every encoder for the chosen codec, graphics card (NVIDIA
  NVENC, AMD AMF, Intel Quick Sync, Direct3D 12, VAAPI, Media Foundation)
  first and then processor (OpenH264, Kvazaar, x264, x265, SVT-AV1, libvpx
  VP9). Those this FFmpeg lacks are marked and not selectable.
- **Test encoders**: tries each encoder on this computer and says plainly which
  work (and why the others don't), checking keyframes and frame order.
- Live-streaming settings for every encoder: no B-frames, a keyframe every two
  seconds, SVT-AV1 in real-time mode, Kvazaar low-delay.

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

**macOS:** unzip and move `NewQ.app` to Applications. The app is signed and
notarised by Apple; the first time, macOS says it was downloaded from the
Internet: choose *Open*.

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
