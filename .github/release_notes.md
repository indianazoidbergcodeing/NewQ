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

Everything this fork has added, newest version first.

### 0.18.5

- **Select several cues:** Ctrl-click (Cmd on a Mac) adds a cue to the selection
  or takes it out; Shift-click selects a run. Delete, Duplicate, Ctrl+Up/Down and
  arm/disarm work on all of them.
- **Drag cues in the cue list:** drag one cue, or the whole selection, to
  reorder; drop on the middle of a group to put it inside. The list scrolls when
  you drag near its top or bottom.
- **Copy, cut and paste cues:** Ctrl+C, Ctrl+X and Ctrl+V, also from the Edit
  menu and the right-click menu. Groups go with their children, a paste can go
  into another show, and a fade, stop or control cue is re-aimed at the copy.
- **Right-click selected cues:** *Group the N selected cues* wraps them in one new
  group; *Move N cues into group* puts them in a group that exists. Right-clicking
  a selected cue keeps the selection.
- **Layers:** every picture cue has a **Layer** number (-1000 to 1000). Layer 1 is
  drawn over layer 0, 2 over 0 and 1, and a negative layer goes under 0. A cue
  that cuts or crossfades in replaces only what is on its own layer of that
  display, so cues on other layers keep playing.
- **Shape cues:** *Cue ▸ Add Shape cue* draws a solid **square, rectangle,
  circle, oval or star** in a colour you choose (with opacity), at the size you
  set, and you can move and scale it like any other picture.
- **Edit several cues at once:** with cues selected, a change in the inspector
  (duration, pre-wait, fades, layer, volume, effects, arming…) is made in all of
  them. Names, numbers, notes, hotkeys, files and text stay each cue's own.
- **Changes apply while a cue plays:** layer, position, scale, rotation, crop,
  colour, effects, opacity, volume, fit, the display, how long a picture is held,
  and a shape's or text's own settings follow at once, with nothing to stop and
  fire again.
- **Groups:** in a *Play in order* group, a picture or video that crossfades in
  now starts before the one before it ends, so the two fade across each other
  instead of dipping to black. Cues of a *Start all together* group share their
  display without replacing each other, and slide shows keep working as before.

### 0.18.4

- **Capture cards on Windows and Linux:** a camera cue set to *Automatic* now
  names the format the device offers, which fixes capture cards such as the
  Magewell USB Capture SDI that failed with "I/O error".
- **Blackmagic output:** a display's *Send out* can go to a Blackmagic UltraStudio
  or DeckLink output (for example an UltraStudio Express or Mini Monitor), which
  does not show up as a display. Needs an FFmpeg built with DeckLink; not tried
  on hardware.
- **Recording:** a display's *Send out* can record the picture to a file: a
  folder (Downloads by default), H.264 or H.265, MKV, MOV or MP4, a bitrate and
  an optional area of the picture. Every start makes a new file named by the day
  and time. MKV is the default, since the others can be left unplayable by a
  sudden stop (the panel says so).
- **Sound in recordings:** a recording can also take what one of the show's audio
  outputs plays and write it with the picture.
- **New interface (optional):** *Window ▸ New interface* swaps the panes for three
  views: **Editor** (cue list and inspector), **Operator** (a big GO and what is
  running) and **Patch** (cues, displays and audio outputs, where a drag from a
  cue to a display or audio output routes it). Switch back any time; the show is
  the same.

### 0.18.3

- **Choose the H.264 encoder by hand:** the streaming server section has an
  Encoder list (Automatic, x264, VideoToolbox, NVENC, Quick Sync, AMF,
  OpenH264), for when a hardware encoder passes the quick test and still sends a
  stream a viewer can't show.
- **AMD AMF fix:** it now sends a keyframe every two seconds instead of one for
  the whole stream, so a viewer who joins late sees a picture.
- **A stuck FFmpeg is ended:** a stream's FFmpeg that stopped answering no longer
  keeps Stop from returning or stays running after NewQ is closed.

### 0.18.2

- **Streaming server locked down:** only this computer can send a picture to the
  server it runs, anyone can watch, and a second sender can't take the stream
  over. A chosen network adapter that isn't there no longer opens the server to
  every adapter.
- **Server runs on its own thread** and acts on a port change once you stop
  typing; several displays running their own server agree on one adapter and
  ports.
- **Test server** says why a test failed (not started, nothing listening, no
  answer) and waits longer.
- **Plays in a browser:** video is sent without B-frames and WebRTC also listens
  on TCP.
- **No leftover servers:** the server ends with NewQ, even if NewQ crashes, and
  one an earlier NewQ left behind is stopped; its output is kept in
  `mediamtx.log`.
- An empty server address is never treated as reachable.

### 0.18.1

- **Streaming server in NewQ:** in a display's *Send out* section, *Streaming
  server (MediaMTX)* can run the server itself (*Run the server in NewQ*).
- **Player link and QR code:** show the web player link and a QR code so phones
  open the stream without typing the address.
- **Network adapter:** choose which network connection the server listens on and
  the links carry.
- **Test server:** checks the server's ports.
