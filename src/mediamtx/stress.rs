//! Stress tests for the streaming path. They need MediaMTX and FFmpeg (as the other
//! real-server tests do), take minutes, and are run on purpose:
//! `NEWQ_MEDIAMTX_DIR=… NEWQ_FFMPEG_DIR=… cargo test --release stress_ -- --ignored --nocapture`
//! `NEWQ_STRESS_SECS` sets the length of the soak (default 120), `NEWQ_STRESS_ENCODER` the encoder.

use super::*;
use crate::media::ffmpeg::Ffmpeg;
use crate::show::{ServerProtocol, StreamKind};
use crate::stream::Sink;
use std::net::Ipv4Addr;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

fn free(n: usize) -> Vec<u16> {
    let socks: Vec<_> = (0..n)
        .map(|_| std::net::TcpListener::bind("127.0.0.1:0").unwrap())
        .collect();
    socks
        .iter()
        .map(|s| s.local_addr().unwrap().port())
        .collect()
}

fn hosting() -> Hosting {
    let p = free(6);
    Hosting {
        listen: Some(Ipv4Addr::LOCALHOST.into()),
        rtsp: p[0],
        rtmp: p[1],
        srt: p[2],
        player: p[3],
        hls: p[4],
        ice: p[5],
        rtp: 0,
    }
}

fn kind(h: &Hosting, path: &str) -> StreamKind {
    let mut k = StreamKind::server(ServerProtocol::Rtsp);
    if let StreamKind::Server {
        port,
        host_here,
        player_port,
        path: p,
        encoder,
        ..
    } = &mut k
    {
        *port = h.rtsp;
        *host_here = true;
        *player_port = h.player;
        *p = path.into();
        *encoder = std::env::var("NEWQ_STRESS_ENCODER").unwrap_or_default();
    }
    k
}

/// A moving picture, so the encoder has real work.
fn frame(size: (u32, u32), n: u32, buf: &mut Vec<u8>) {
    buf.clear();
    buf.reserve((size.0 * size.1 * 4) as usize);
    for y in 0..size.1 {
        for x in 0..size.0 {
            let v = ((x + n * 7) ^ (y + n * 3)) as u8;
            buf.extend_from_slice(&[v, v.wrapping_add(80), x as u8, 255]);
        }
    }
}

/// FFmpeg processes whose command line mentions `needle`.
fn ffmpeg_with(needle: &str) -> Vec<u32> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_memory(),
    );
    sys.processes()
        .values()
        .filter(|p| {
            p.name().to_string_lossy().to_lowercase().contains("ffmpeg")
                && p.cmd().iter().any(|a| a.to_string_lossy().contains(needle))
        })
        .map(|p| p.pid().as_u32())
        .collect()
}

fn wait_port(port: u16) {
    assert!((0..100).any(|_| {
        std::thread::sleep(Duration::from_millis(100));
        crate::streamlink::tcp_probe("127.0.0.1", port, Duration::from_millis(200)).is_ok()
    }));
}

#[test]
#[ignore]
fn stress_start_stop_cycles_leave_no_ffmpeg_behind() {
    let (Some(exe), Some(ff)) = (locate(), Ffmpeg::locate()) else {
        eprintln!("needs MediaMTX and FFmpeg: skipped");
        return;
    };
    let dir = std::env::temp_dir().join(format!("newq-stress-a-{}", std::process::id()));
    let h = hosting();
    let mut server = Hosted::start(&exe, &dir, h).expect("start");
    wait_port(h.rtsp);
    let tag = format!("cyc{}", std::process::id());
    let size = (1280u32, 720u32);
    let mut buf = Vec::new();
    let mut worst_start = Duration::ZERO;
    let mut failures = Vec::new();
    for i in 0..60u32 {
        let k = kind(&h, &tag);
        let t = Instant::now();
        let mut sink = match Sink::start(&ff, size, 30, &k) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("cycle {i}: start failed: {e}"));
                continue;
            }
        };
        worst_start = worst_start.max(t.elapsed());
        let frames = 5 + (i % 40);
        for n in 0..frames {
            frame(size, n, &mut buf);
            if let Err(e) = sink.write(&buf) {
                failures.push(format!("cycle {i}: write {n}: {e}"));
                break;
            }
        }
        match i % 4 {
            // stopped by the user
            0 => drop(sink),
            // closed cleanly
            1 => sink.finish(),
            3 => {
                // the server goes away under it, then Stop closes the sink gracefully
                let _ = server.child.kill();
                let _ = server.child.wait();
                let t = Instant::now();
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    sink.finish();
                    let _ = tx.send(());
                });
                let done = rx.recv_timeout(Duration::from_secs(30)).is_ok();
                eprintln!(
                    "cycle {i}: finish() after server loss: done={done} in {:?}",
                    t.elapsed()
                );
                if !done {
                    failures.push(format!("cycle {i}: finish() hung for 30 s"));
                }
                server = Hosted::start(&exe, &dir, h).expect("restart server");
                wait_port(h.rtsp);
            }
            _ => {
                // the server goes away under it, then NewQ drops the sink
                let _ = server.child.kill();
                let _ = server.child.wait();
                for n in 0..20 {
                    frame(size, n, &mut buf);
                    if sink.write(&buf).is_err() {
                        break;
                    }
                }
                drop(sink);
                server = Hosted::start(&exe, &dir, h).expect("restart server");
                wait_port(h.rtsp);
            }
        }
        let left = ffmpeg_with(&tag);
        if !left.is_empty() {
            failures.push(format!(
                "cycle {i} (mode {}): ffmpeg left behind: {left:?}",
                i % 4
            ));
        }
    }
    eprintln!("worst Sink::start: {worst_start:?}");
    eprintln!("failures: {}", failures.len());
    for f in &failures {
        eprintln!("  {f}");
    }
    drop(server);
    std::fs::remove_dir_all(&dir).ok();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
#[ignore]
fn stress_soak_with_viewers_tracks_memory() {
    let (Some(exe), Some(ff)) = (locate(), Ffmpeg::locate()) else {
        eprintln!("needs MediaMTX and FFmpeg: skipped");
        return;
    };
    let secs: u64 = std::env::var("NEWQ_STRESS_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    let dir = std::env::temp_dir().join(format!("newq-stress-b-{}", std::process::id()));
    let h = hosting();
    let server = Hosted::start(&exe, &dir, h).expect("start");
    let server_pid = server.child.id();
    wait_port(h.rtsp);
    let tag = format!("soak{}", std::process::id());
    let k = kind(&h, &tag);
    let size = (1920u32, 1080u32);
    let mut sink = Sink::start(&ff, size, 30, &k).expect("start the sink");
    let url = |s: &str| format!("{s}{tag}");
    let spawn_viewer = |u: String, rtsp: bool| {
        let mut c = ff.command();
        if rtsp {
            c.args(["-rtsp_transport", "tcp"]);
        }
        c.arg("-i").arg(u).args(["-f", "null", "-"]);
        c.stdout(Stdio::null()).stderr(Stdio::null());
        c.spawn().expect("viewer")
    };
    let mut viewers: Vec<std::process::Child> = Vec::new();
    let mut buf = Vec::new();
    let start = Instant::now();
    let mut n = 0u32;
    let mut worst_write = Duration::ZERO;
    let mut late = 0u32;
    let mut sys = System::new();
    let me = sysinfo::get_current_pid().unwrap();
    let mut samples: Vec<(u64, Vec<(String, u64)>)> = Vec::new();
    let mut next_sample = Instant::now();
    let mut churn_viewer: Option<std::process::Child> = None;
    let mut churn_at = Instant::now();
    let mut error = None;
    while start.elapsed() < Duration::from_secs(secs) {
        let due = start + Duration::from_micros(n as u64 * 33_333);
        // Viewers join two seconds in, after the first frames have reached the server.
        if viewers.is_empty() && n == 60 {
            for _ in 0..3 {
                viewers.push(spawn_viewer(
                    url(&format!("rtsp://127.0.0.1:{}/", h.rtsp)),
                    true,
                ));
            }
            viewers.push(spawn_viewer(
                url(&format!("rtmp://127.0.0.1:{}/", h.rtmp)),
                false,
            ));
            viewers.push(spawn_viewer(
                format!("http://127.0.0.1:{}/{tag}/index.m3u8", h.hls),
                false,
            ));
        }
        // One more viewer that comes and goes every seven seconds.
        if n > 60 && churn_at.elapsed() > Duration::from_secs(7) {
            if let Some(mut c) = churn_viewer.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            churn_viewer = Some(spawn_viewer(
                url(&format!("rtsp://127.0.0.1:{}/", h.rtsp)),
                true,
            ));
            churn_at = Instant::now();
        }
        frame(size, n, &mut buf);
        let t = Instant::now();
        if let Err(e) = sink.write(&buf) {
            error = Some(format!("write {n}: {e}"));
            break;
        }
        worst_write = worst_write.max(t.elapsed());
        n += 1;
        let now = Instant::now();
        if now < due {
            std::thread::sleep(due - now);
        } else if now - due > Duration::from_millis(100) {
            late += 1;
        }
        if Instant::now() >= next_sample {
            next_sample += Duration::from_secs(5);
            sys.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing()
                    .with_cmd(UpdateKind::Always)
                    .with_memory(),
            );
            let mut row = Vec::new();
            let mut add = |name: &str, pid: u32| {
                if let Some(p) = sys.process(sysinfo::Pid::from_u32(pid)) {
                    row.push((name.to_string(), p.memory()));
                }
            };
            add("test", me.as_u32());
            add("mediamtx", server_pid);
            for (i, v) in viewers.iter().enumerate() {
                add(&format!("viewer{i}"), v.id());
            }
            // The publisher is the FFmpeg that was reading raw frames.
            for pid in ffmpeg_with("rawvideo") {
                add("publisher", pid);
            }
            samples.push((start.elapsed().as_secs(), row));
        }
    }
    let secs_run = start.elapsed().as_secs_f64();
    eprintln!(
        "sent {n} frames in {secs_run:.1}s = {:.2} fps; worst write {worst_write:?}; frames more than 100 ms late: {late}",
        n as f64 / secs_run
    );
    if let Some(e) = &error {
        eprintln!("STREAM ERROR: {e}");
    }
    eprintln!("memory (MB) by process, every 5 s:");
    for (t, row) in &samples {
        let cols: Vec<String> = row
            .iter()
            .map(|(n, b)| format!("{n}={:.0}", *b as f64 / 1048576.0))
            .collect();
        eprintln!("  t={t:>4}s  {}", cols.join("  "));
    }
    // Growth: the median of the first third against the last third, per process.
    if samples.len() >= 6 {
        let names: Vec<String> = samples[0].1.iter().map(|(n, _)| n.clone()).collect();
        for name in names {
            let series: Vec<f64> = samples
                .iter()
                .filter_map(|(_, r)| {
                    r.iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, b)| *b as f64 / 1048576.0)
                })
                .collect();
            if series.len() < 6 {
                continue;
            }
            let third = series.len() / 3;
            let med = |s: &[f64]| {
                let mut v = s.to_vec();
                v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                v[v.len() / 2]
            };
            eprintln!(
                "  {name}: first third {:.0} MB -> last third {:.0} MB",
                med(&series[..third]),
                med(&series[series.len() - third..])
            );
        }
    }
    let dead_viewers = viewers
        .iter_mut()
        .filter_map(|v| v.try_wait().ok().flatten())
        .count();
    eprintln!(
        "viewers that ended on their own: {dead_viewers} of {}",
        viewers.len()
    );
    for mut v in viewers {
        let _ = v.kill();
        let _ = v.wait();
    }
    if let Some(mut c) = churn_viewer {
        let _ = c.kill();
        let _ = c.wait();
    }
    drop(sink);
    std::thread::sleep(Duration::from_millis(500));
    let left = ffmpeg_with(&tag);
    eprintln!("ffmpeg left behind after the soak: {left:?}");
    drop(server);
    std::fs::remove_dir_all(&dir).ok();
    assert!(error.is_none(), "{error:?}");
    assert!(left.is_empty(), "{left:?}");
}
