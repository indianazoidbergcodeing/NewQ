//! Running MediaMTX (the streaming server) inside NewQ, so a display can be
//! sent out and watched on phones and other computers with nothing else to
//! install. NewQ ships the `mediamtx` program next to itself, the way it ships
//! FFmpeg, runs it as a child process with a configuration it writes, and
//! stops it when it is no longer wanted or NewQ quits.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "mediamtx.exe"
    } else {
        "mediamtx"
    }
}

/// Where the program is: `$NEWQ_MEDIAMTX_DIR`, next to NewQ's own executable
/// (and its `mediamtx/` folder), `third_party/mediamtx` while developing, then
/// `PATH`.
pub fn locate() -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(d) = std::env::var("NEWQ_MEDIAMTX_DIR") {
        dirs.push(PathBuf::from(d));
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        dirs.push(exe_dir.join("mediamtx"));
        dirs.push(exe_dir.clone());
        if let Some(root) = exe_dir.parent().and_then(Path::parent) {
            dirs.push(root.join("third_party").join("mediamtx"));
        }
    }
    dirs.push(PathBuf::from("third_party").join("mediamtx"));
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter()
        .map(|d| d.join(exe_name()))
        .find(|p| p.is_file())
}

/// Stop a server left running by an earlier NewQ that was killed or crashed: it
/// would still hold the ports, and the new server could not start. Only a
/// program started with this very configuration file is stopped, so a MediaMTX
/// of anyone's own is left alone.
fn stop_leftovers(conf: &Path) {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing().with_cmd(sysinfo::UpdateKind::Always),
    );
    let mut stopped = false;
    for p in sys.processes().values() {
        let ours = p.cmd().iter().any(|a| Path::new(a) == conf);
        if ours && p.pid().as_u32() != std::process::id() && p.kill() {
            log::info!(
                "stopped a MediaMTX left running by an earlier NewQ (pid {})",
                p.pid()
            );
            stopped = true;
        }
    }
    if stopped {
        // Let the system free its ports.
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// What the hosted server is asked to do: which address to listen on and the
/// port of each way in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hosting {
    /// The adapter's address to listen on; `None` listens on every adapter.
    pub listen: Option<IpAddr>,
    pub rtsp: u16,
    pub rtmp: u16,
    pub srt: u16,
    /// The web player (WebRTC).
    pub player: u16,
    /// The same picture as HLS, for browsers that can't do WebRTC.
    pub hls: u16,
    /// Where WebRTC's media travels (UDP, and TCP as a fallback).
    pub ice: u16,
    /// The UDP port RTSP viewers can receive media on (RTCP uses the next one).
    /// 0 turns UDP off: RTSP then runs over TCP only.
    pub rtp: u16,
}

impl Hosting {
    /// MediaMTX's own usual ports, on `listen`.
    pub fn standard(listen: Option<IpAddr>) -> Hosting {
        Hosting {
            listen,
            rtsp: 8554,
            rtmp: 1935,
            srt: 8890,
            player: crate::show::default_player_port(),
            hls: 8888,
            ice: 8189,
            rtp: 8000,
        }
    }

    /// The configuration file's text. Only what NewQ uses is on: no control
    /// API, metrics or recording, and publishing is open to anything on the
    /// listening address (it is for a show network, not the internet).
    pub fn config(&self) -> String {
        let at = |port: u16| match self.listen {
            Some(IpAddr::V6(ip)) => format!("\"[{ip}]:{port}\""),
            Some(ip) => format!("\"{ip}:{port}\""),
            None => format!("\":{port}\""),
        };
        // WebRTC offers its own addresses to the browser: with an adapter
        // chosen, offer only that one.
        let webrtc_hosts = match self.listen {
            Some(ip) => format!("webrtcIPsFromInterfaces: no\nwebrtcAdditionalHosts: [\"{ip}\"]\n"),
            None => String::new(),
        };
        let rtsp_transports = if self.rtp == 0 {
            "rtspTransports: [tcp]\n".to_string()
        } else {
            format!(
                "rtspTransports: [udp, tcp]\nrtpAddress: {}\nrtcpAddress: {}\n",
                at(self.rtp),
                at(self.rtp + 1)
            )
        };
        format!(
            "# Written by NewQ each time it starts the server: changes here are lost.\n\
             logLevel: info\n\
             api: no\n\
             metrics: no\n\
             pprof: no\n\
             playback: no\n\
             moq: no\n\
             {rtsp_transports}\
             rtspAddress: {rtsp}\n\
             rtspsAddress: \"\"\n\
             rtmpAddress: {rtmp}\n\
             rtmpsAddress: \"\"\n\
             srtAddress: {srt}\n\
             hlsAddress: {hls}\n\
             webrtcAddress: {player}\n\
             webrtcLocalUDPAddress: {udp}\n\
             webrtcLocalTCPAddress: {udp}\n\
             {webrtc_hosts}\
             paths:\n  all_others:\n",
            rtsp = at(self.rtsp),
            rtmp = at(self.rtmp),
            srt = at(self.srt),
            hls = at(self.hls),
            player = at(self.player),
            udp = at(self.ice),
        )
    }
}

/// The server NewQ started. Dropping it stops the program.
pub struct Hosted {
    child: Child,
    pub hosting: Hosting,
    /// The last lines it wrote, for saying why it stopped.
    tail: Arc<Mutex<VecDeque<String>>>,
}

/// How the hosted server is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    /// It stopped by itself: the end of what it said (a port in use shows here).
    Stopped(String),
}

impl Hosted {
    /// Write the configuration to `dir` and start the program.
    pub fn start(exe: &Path, dir: &Path, hosting: Hosting) -> Result<Hosted, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let conf = dir.join("mediamtx.yml");
        stop_leftovers(&conf);
        std::fs::write(&conf, hosting.config()).map_err(|e| format!("{}: {e}", conf.display()))?;
        // What the server says is kept in a file, for finding out why a
        // viewer sees nothing.
        let log = std::fs::File::create(dir.join("mediamtx.log")).ok();
        let mut cmd = Command::new(exe);
        cmd.arg(&conf)
            // It leaves a certificate it makes in its working folder.
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::media::ffmpeg::no_console(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("can't start MediaMTX: {e}"))?;
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let pipes: [Option<Box<dyn std::io::Read + Send>>; 2] = [
            child.stdout.take().map(|p| Box::new(p) as _),
            child.stderr.take().map(|p| Box::new(p) as _),
        ];
        for pipe in pipes.into_iter().flatten() {
            let tail = tail.clone();
            let log = log.as_ref().and_then(|f| f.try_clone().ok());
            std::thread::Builder::new()
                .name("mediamtx-log".into())
                .spawn(move || {
                    for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                        if let Some(f) = &log {
                            let _ = writeln!(&*f, "{line}");
                        }
                        let mut t = tail.lock();
                        if t.len() >= 12 {
                            t.pop_front();
                        }
                        t.push_back(line);
                    }
                })
                .ok();
        }
        Ok(Hosted {
            child,
            hosting,
            tail,
        })
    }

    pub fn state(&mut self) -> State {
        match self.child.try_wait() {
            Ok(None) => State::Running,
            Ok(Some(status)) => State::Stopped(self.why(Some(status.to_string()))),
            Err(e) => State::Stopped(e.to_string()),
        }
    }

    /// The error line it ended with, if it said one, else `fallback`.
    fn why(&self, fallback: Option<String>) -> String {
        let t = self.tail.lock();
        t.iter()
            .rev()
            .find(|l| l.contains("ERR") || l.contains("error") || l.contains("address already"))
            .or(t.back())
            .map(|l| l.trim().to_string())
            .or(fallback)
            .unwrap_or_else(|| "it stopped".into())
    }
}

impl Drop for Hosted {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, TcpListener};

    /// A port free for TCP and UDP (WebRTC uses both), never given out twice:
    /// tests run in parallel, and a port freed again is free for the next caller.
    fn free_port() -> u16 {
        static GIVEN: parking_lot::Mutex<Vec<u16>> = parking_lot::Mutex::new(Vec::new());
        let mut given = GIVEN.lock();
        loop {
            let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = tcp.local_addr().unwrap().port();
            if !given.contains(&port) && std::net::UdpSocket::bind(("127.0.0.1", port)).is_ok() {
                given.push(port);
                return port;
            }
        }
    }

    #[test]
    fn the_configuration_listens_on_the_chosen_adapter_only() {
        let h = Hosting::standard(Some(Ipv4Addr::new(192, 168, 1, 20).into()));
        let c = h.config();
        assert!(c.contains("rtspAddress: \"192.168.1.20:8554\""), "{c}");
        assert!(c.contains("webrtcAddress: \"192.168.1.20:8889\""), "{c}");
        assert!(
            c.contains("webrtcAdditionalHosts: [\"192.168.1.20\"]"),
            "{c}"
        );
        assert!(c.contains("webrtcIPsFromInterfaces: no"), "{c}");
        assert!(c.contains("paths:\n  all_others:"), "{c}");
    }

    #[test]
    fn without_an_adapter_it_listens_on_all_of_them() {
        let c = Hosting::standard(None).config();
        assert!(
            c.contains("rtspAddress: \":8554\"") && c.contains("srtAddress: \":8890\""),
            "{c}"
        );
        assert!(!c.contains("webrtcAdditionalHosts"), "{c}");
    }

    #[test]
    fn an_ipv6_adapter_is_bracketed() {
        let h = Hosting::standard(Some("fd00::5".parse().unwrap()));
        assert!(h.config().contains("rtspAddress: \"[fd00::5]:8554\""));
    }

    /// With a real MediaMTX (set NEWQ_MEDIAMTX_DIR, or have it on the PATH):
    /// start it, find its ports, and stop it.
    #[test]
    fn a_real_server_starts_listens_and_stops() {
        let Some(exe) = locate() else {
            eprintln!("no MediaMTX here: skipped");
            return;
        };
        let dir = std::env::temp_dir().join(format!("newq-mtx-run-{}", std::process::id()));
        let lo: IpAddr = Ipv4Addr::LOCALHOST.into();
        let hosting = Hosting {
            listen: Some(lo),
            rtsp: free_port(),
            rtmp: free_port(),
            srt: free_port(),
            player: free_port(),
            hls: free_port(),
            ice: free_port(),
            rtp: 0,
        };
        let mut server = Hosted::start(&exe, &dir, hosting).expect("start");
        let up = (0..50).any(|_| {
            std::thread::sleep(Duration::from_millis(100));
            crate::streamlink::tcp_probe("127.0.0.1", hosting.rtsp, Duration::from_millis(200))
                .is_ok()
        });
        assert!(up, "the RTSP port never opened: {:?}", server.state());
        assert_eq!(server.state(), State::Running);
        let probes = crate::streamlink::probe_server(
            crate::show::ServerProtocol::Rtsp,
            "127.0.0.1",
            hosting.rtsp,
            hosting.player,
        );
        assert!(probes.iter().all(|p| p.result.is_ok()), "{probes:?}");
        // A second server on the same ports says why it stopped.
        let mut clash = Hosted::start(&exe, &dir.join("clash"), hosting).expect("start");
        let stopped = (0..50).any(|_| {
            std::thread::sleep(Duration::from_millis(100));
            matches!(clash.state(), State::Stopped(_))
        });
        assert!(stopped, "a second server on the same ports kept running");
        drop(server);
        assert!(
            crate::streamlink::tcp_probe("127.0.0.1", hosting.rtsp, Duration::from_millis(300))
                .is_err()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A server left running by a NewQ that was killed doesn't stop the next
    /// one from starting.
    #[test]
    fn a_server_left_behind_is_stopped_before_the_next_starts() {
        let Some(exe) = locate() else {
            eprintln!("no MediaMTX here: skipped");
            return;
        };
        let dir = std::env::temp_dir().join(format!("newq-mtx-left-{}", std::process::id()));
        let hosting = Hosting {
            listen: Some(Ipv4Addr::LOCALHOST.into()),
            rtsp: free_port(),
            rtmp: free_port(),
            srt: free_port(),
            player: free_port(),
            hls: free_port(),
            ice: free_port(),
            rtp: 0,
        };
        let first = Hosted::start(&exe, &dir, hosting).expect("start");
        assert!((0..50).any(|_| {
            std::thread::sleep(Duration::from_millis(100));
            crate::streamlink::tcp_probe("127.0.0.1", hosting.rtsp, Duration::from_millis(200))
                .is_ok()
        }));
        // NewQ dying leaves the program running, which forgetting the handle imitates.
        std::mem::forget(first);
        let mut second = Hosted::start(&exe, &dir, hosting).expect("start");
        std::thread::sleep(Duration::from_millis(1500));
        assert_eq!(
            second.state(),
            State::Running,
            "the second server could not start"
        );
        drop(second);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The whole chain with the real programs: NewQ runs MediaMTX, publishes a
    /// picture to it through FFmpeg, the web player page answers, and the
    /// picture can be read back over RTSP.
    #[test]
    fn a_stream_published_to_the_hosted_server_can_be_watched() {
        use crate::media::ffmpeg::Ffmpeg;
        use crate::show::{ServerProtocol, StreamKind};
        use crate::stream::{Sink, encoder};
        use std::io::{Read, Write};
        let (Some(exe), Some(ff)) = (locate(), Ffmpeg::locate()) else {
            eprintln!("needs MediaMTX and FFmpeg: skipped");
            return;
        };
        if encoder(&ff).is_none() {
            eprintln!("no H.264 encoder: skipped");
            return;
        }
        let dir = std::env::temp_dir().join(format!("newq-mtx-e2e-{}", std::process::id()));
        let hosting = Hosting {
            listen: Some(Ipv4Addr::LOCALHOST.into()),
            rtsp: free_port(),
            rtmp: free_port(),
            srt: free_port(),
            player: free_port(),
            hls: free_port(),
            ice: free_port(),
            rtp: 0,
        };
        let mut server = Hosted::start(&exe, &dir, hosting).expect("start");
        assert!((0..50).any(|_| {
            std::thread::sleep(Duration::from_millis(100));
            crate::streamlink::tcp_probe("127.0.0.1", hosting.rtsp, Duration::from_millis(200))
                .is_ok()
        }));
        let mut kind = StreamKind::server(ServerProtocol::Rtsp);
        if let StreamKind::Server {
            port,
            host_here,
            player_port,
            ..
        } = &mut kind
        {
            *port = hosting.rtsp;
            *host_here = true;
            *player_port = hosting.player;
        }
        let size = (320u32, 180u32);
        let frame = [40u8, 120, 200, 255].repeat((size.0 * size.1) as usize);
        let mut sink = Sink::start(&ff, size, 25, &kind).expect("start the sink");
        // Keep sending while the page and the reader are tried.
        let sender = std::thread::spawn(move || {
            for _ in 0..200 {
                if sink.write(&frame).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            sink.finish();
        });
        // The web player page the QR code opens.
        let mut page = String::new();
        let mut conn = std::net::TcpStream::connect(("127.0.0.1", hosting.player)).unwrap();
        conn.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(conn, "GET /newq/ HTTP/1.0\r\nHost: x\r\n\r\n").unwrap();
        conn.read_to_string(&mut page).ok();
        assert!(
            page.starts_with("HTTP/1.") && page.contains(" 200 "),
            "{}",
            &page[..page.len().min(200)]
        );
        // Read the picture back the way a player would.
        let mut got = false;
        for _ in 0..10 {
            let out = ff
                .command()
                .args(["-rtsp_transport", "tcp", "-i"])
                .arg(format!("rtsp://127.0.0.1:{}/newq", hosting.rtsp))
                .args(["-frames:v", "5", "-f", "null", "-"])
                .output()
                .unwrap();
            if out.status.success() {
                got = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        assert!(got, "the stream never played back: {:?}", server.state());
        sender.join().ok();
        drop(server);
        std::fs::remove_dir_all(&dir).ok();
    }
}
