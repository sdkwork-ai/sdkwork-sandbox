// demos 能力探针 —— Rust 实现。协议见 demos/SPEC.md。
//
// 模式（照抄 SPEC §4，跨语言必须同名）：
//   默认              跑全部检查
//   --echo-child      往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
//   --read-stdin      读一行 stdin，内容为 ping 时回 pong
//   --argv-probe A B  回显 argv（argv-probe:2:A:B）
//
// 只用 std：本探针**不引入任何 crate**（`rustc -O` 直接编译，没有 Cargo.toml、
// 不碰 registry）。因此 Tier B 里标准库确实没有的能力（SHA-256 / Base64 / JSON /
// 正则 / gzip）如实记 SKIP，而不是悄悄用一个第三方 crate 糊过去 —— 那会让
// "这台机器的运行环境具备什么"这个问题失去意义。
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const UNICODE_S: &str = "中文-日本語-한국어-🚀";
const UNICODE_CODEPOINTS: usize = 12;
const UNICODE_BYTES: usize = 31;
const LARGE_BYTES: usize = 262144;
const MAX_DEPTH: i32 = 3;

struct Report {
    pass: i32,
    fail: i32,
    skip: i32,
}

impl Report {
    fn record(&mut self, cid: &str, status: &str, detail: &str) {
        let clean: String = detail
            .chars()
            .map(|c| if c == '\t' || c == '\r' || c == '\n' { ' ' } else { c })
            .take(160)
            .collect();
        match status {
            "PASS" => self.pass += 1,
            "FAIL" => self.fail += 1,
            _ => self.skip += 1,
        }
        println!("CAP {} {} {}", cid, status, clean);
        let _ = std::io::stdout().flush();
    }
}

/// 检查的三种结局。SKIP 必须带理由（SPEC §3）。
enum Outcome {
    Pass(String),
    Skip(String),
}

type Check = fn(&i32) -> Result<Outcome, String>;

fn self_exe() -> Result<String, String> {
    env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("current_exe: {}", e))
}

fn depth_of() -> i32 {
    env::var("DEMO_DEPTH")
        .ok()
        .and_then(|v| v.trim().parse::<i32>().ok())
        .unwrap_or(0)
}

/// 启动本程序自身，返回 (退出码, stdout, stderr)。
fn spawn(args: &[&str], stdin_data: Option<&str>, depth: i32) -> Result<(i32, String, String), String> {
    let exe = self_exe()?;
    let mut cmd = Command::new(exe);
    cmd.args(args);
    cmd.env("DEMO_DEPTH", (depth + 1).to_string());
    cmd.stdin(if stdin_data.is_some() { Stdio::piped() } else { Stdio::null() });
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("spawn: {}", e))?;
    if let Some(data) = stdin_data {
        if let Some(mut si) = child.stdin.take() {
            si.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
        }
    }
    let out = child.wait_with_output().map_err(|e| format!("wait: {}", e))?;
    Ok((
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    ))
}

// ------------------------------------------------------------------ Tier A
fn a01(depth: &i32) -> Result<Outcome, String> {
    let (_, out, _) = spawn(&["--argv-probe", "alpha", "beta"], None, *depth)?;
    let got = out.trim();
    if got != "argv-probe:2:alpha:beta" {
        return Err(format!("argv 回显不符: {}", got));
    }
    Ok(Outcome::Pass("2 args round-tripped".into()))
}

fn a02(_depth: &i32) -> Result<Outcome, String> {
    match env::var("DEMO_LANG_TAG") {
        Ok(v) if v == "demos-capability" => Ok(Outcome::Pass("env visible".into())),
        other => Err(format!("DEMO_LANG_TAG={:?}", other)),
    }
}

fn a03(depth: &i32) -> Result<Outcome, String> {
    let (_, out, err) = spawn(&["--echo-child"], None, *depth)?;
    if out.trim() != "child-ok" {
        return Err(format!("stdout={:?}", out.trim()));
    }
    if !err.contains("child-err") {
        return Err(format!("stderr={:?}", err.trim()));
    }
    if out.contains("child-err") {
        return Err("stderr leaked into stdout".into());
    }
    Ok(Outcome::Pass("stdout/stderr separated".into()))
}

fn a04(depth: &i32) -> Result<Outcome, String> {
    let (_, out, _) = spawn(&["--read-stdin"], Some("ping\n"), *depth)?;
    if out.trim() != "pong" {
        return Err(format!("reply={:?}", out.trim()));
    }
    Ok(Outcome::Pass("ping->pong".into()))
}

fn a05(_depth: &i32) -> Result<Outcome, String> {
    fs::write("a05.txt", "hello-io").map_err(|e| e.to_string())?;
    let got = fs::read_to_string("a05.txt").map_err(|e| e.to_string())?;
    if got != "hello-io" {
        return Err(format!("read back {:?}", got));
    }
    Ok(Outcome::Pass("text round-trip ok".into()))
}

fn a06(_depth: &i32) -> Result<Outcome, String> {
    fs::write("a06.txt", "a").map_err(|e| e.to_string())?;
    let mut fh = fs::OpenOptions::new()
        .append(true)
        .open("a06.txt")
        .map_err(|e| e.to_string())?;
    fh.write_all(b"b").map_err(|e| e.to_string())?;
    drop(fh);
    let got = fs::read_to_string("a06.txt").map_err(|e| e.to_string())?;
    if got != "ab" {
        return Err(format!("append 结果 {:?}", got));
    }
    Ok(Outcome::Pass("append ok".into()))
}

fn a07(_depth: &i32) -> Result<Outcome, String> {
    let payload: Vec<u8> = (0..256u32).map(|i| i as u8).collect();
    fs::write("a07.bin", &payload).map_err(|e| e.to_string())?;
    let got = fs::read("a07.bin").map_err(|e| e.to_string())?;
    if got != payload {
        return Err(format!("二进制往返不一致 len={}", got.len()));
    }
    Ok(Outcome::Pass("256 bytes incl 0x00/0xFF".into()))
}

fn a08(_depth: &i32) -> Result<Outcome, String> {
    fs::write("a08.txt", "statted").map_err(|e| e.to_string())?;
    let meta = fs::metadata("a08.txt").map_err(|e| e.to_string())?;
    if meta.len() != 7 {
        return Err(format!("size={}", meta.len()));
    }
    fs::remove_file("a08.txt").map_err(|e| e.to_string())?;
    if Path::new("a08.txt").exists() {
        return Err("remove failed".into());
    }
    Ok(Outcome::Pass("size=7 then removed".into()))
}

fn a09(_depth: &i32) -> Result<Outcome, String> {
    let dir = "a09dir";
    let _ = fs::remove_file(format!("{}/inner.txt", dir));
    let _ = fs::remove_dir(dir);
    fs::create_dir(dir).map_err(|e| e.to_string())?;
    fs::write(format!("{}/inner.txt", dir), "x").map_err(|e| e.to_string())?;
    let mut found = false;
    for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
        if e.map_err(|e| e.to_string())?.file_name().to_string_lossy() == "inner.txt" {
            found = true;
        }
    }
    if !found {
        return Err("listing missing inner.txt".into());
    }
    fs::remove_file(format!("{}/inner.txt", dir)).map_err(|e| e.to_string())?;
    fs::remove_dir(dir).map_err(|e| e.to_string())?;
    if Path::new(dir).exists() {
        return Err("rmdir failed".into());
    }
    Ok(Outcome::Pass("mkdir/list/rmdir ok".into()))
}

fn a10(_depth: &i32) -> Result<Outcome, String> {
    // std 没有临时文件 API：用「pid + 纳秒」拼一个唯一名，再以 create_new 保证不覆盖。
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = format!("demos-{}-{}.tmp", std::process::id(), nanos);
    let mut fh = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    fh.write_all(b"temp-content").map_err(|e| e.to_string())?;
    drop(fh);
    let got = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&path);
    if got != "temp-content" {
        return Err(format!("temp read {:?}", got));
    }
    Ok(Outcome::Pass("unique temp file (create_new)".into()))
}

fn a11(_depth: &i32) -> Result<Outcome, String> {
    let cps = UNICODE_S.chars().count();
    if cps != UNICODE_CODEPOINTS {
        return Err(format!("codepoints={}", cps));
    }
    if UNICODE_S.len() != UNICODE_BYTES {
        return Err(format!("bytes={}", UNICODE_S.len()));
    }
    fs::write("a11.txt", UNICODE_S).map_err(|e| e.to_string())?;
    let got = fs::read_to_string("a11.txt").map_err(|e| e.to_string())?;
    if got != UNICODE_S {
        return Err("round-trip mismatch".into());
    }
    Ok(Outcome::Pass(format!(
        "{} codepoints / {} bytes",
        UNICODE_CODEPOINTS, UNICODE_BYTES
    )))
}

fn a12(_depth: &i32) -> Result<Outcome, String> {
    let mut payload = Vec::with_capacity(LARGE_BYTES);
    while payload.len() < LARGE_BYTES {
        payload.extend_from_slice(b"abcdefgh");
    }
    fs::write("a12.bin", &payload).map_err(|e| e.to_string())?;
    let got = fs::read("a12.bin").map_err(|e| e.to_string())?;
    if got.len() != LARGE_BYTES || got != payload {
        return Err(format!("len={}", got.len()));
    }
    Ok(Outcome::Pass(format!("{} bytes ok", LARGE_BYTES)))
}

fn a13(_depth: &i32) -> Result<Outcome, String> {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64;
    if ms < 1_577_836_800_000 {
        return Err(format!("epoch={}", ms));
    }
    let t0 = Instant::now();
    thread::sleep(Duration::from_millis(50));
    let delta = t0.elapsed().as_secs_f64() * 1000.0;
    if delta < 40.0 {
        return Err(format!("sleep 只测到 {:.1} ms", delta));
    }
    Ok(Outcome::Pass(format!("sleep {:.0}ms", delta)))
}

fn a14(_depth: &i32) -> Result<Outcome, String> {
    // std 没有 RNG：用 SystemTime 做种子的 xorshift64*，够本检查用。
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos() as u64;
    let mut x = seed | 1;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    let v = (x.wrapping_mul(0x2545F4914F6CDD1D) >> 33) % 1000;
    if v >= 1000 {
        return Err(format!("out of range {}", v));
    }
    Ok(Outcome::Pass("in [0,1000) (xorshift, std 无 RNG)".into()))
}

fn a15(_depth: &i32) -> Result<Outcome, String> {
    let mut data = vec![5, 3, 9, 1, 7, 3];
    data.sort_unstable();
    if data != vec![1, 3, 3, 5, 7, 9] {
        return Err(format!("sorted={:?}", data));
    }
    let mut m: HashMap<&str, i32> = HashMap::new();
    m.insert("a", 1);
    m.insert("b", 2);
    if m.get("a") != Some(&1) || m.get("b") != Some(&2) || m.len() != 2 {
        return Err("map broken".into());
    }
    Ok(Outcome::Pass("sort + map ok".into()))
}

fn a16(_depth: &i32) -> Result<Outcome, String> {
    match fs::read_to_string("definitely-missing-file-xyz") {
        Err(e) => Ok(Outcome::Pass(format!("{:?} caught", e.kind()))),
        Ok(_) => Err("no error raised for missing file".into()),
    }
}

fn a17(depth: &i32) -> Result<Outcome, String> {
    let (code, out, _) = spawn(&["--echo-child"], None, *depth)?;
    if code != 0 {
        return Err(format!("child exit={}", code));
    }
    if out.trim() != "child-ok" {
        return Err(format!("child stdout={:?}", out.trim()));
    }
    Ok(Outcome::Pass("child exit=0, stdout captured".into()))
}

fn a18(_depth: &i32) -> Result<Outcome, String> {
    let total = Arc::new(AtomicI32::new(0));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let t = Arc::clone(&total);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                t.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }
    for h in handles {
        h.join().map_err(|_| "thread panicked".to_string())?;
    }
    let got = total.load(Ordering::SeqCst);
    if got != 2000 {
        return Err(format!("total={}", got));
    }
    Ok(Outcome::Pass("2 threads -> 2000".into()))
}

fn a19(_depth: &i32) -> Result<Outcome, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;

    let server = thread::spawn(move || -> Result<String, String> {
        let (mut conn, _) = listener.accept().map_err(|e| e.to_string())?;
        let mut buf = [0u8; 64];
        let n = conn.read(&mut buf).map_err(|e| e.to_string())?;
        let recv = String::from_utf8_lossy(&buf[..n]).to_string();
        conn.write_all(b"tcp-pong").map_err(|e| e.to_string())?;
        conn.flush().map_err(|e| e.to_string())?;
        Ok(recv)
    });

    let mut cli = TcpStream::connect(addr).map_err(|e| e.to_string())?;
    cli.write_all(b"tcp-ping").map_err(|e| e.to_string())?;
    cli.flush().map_err(|e| e.to_string())?;
    let mut buf = [0u8; 64];
    let n = cli.read(&mut buf).map_err(|e| e.to_string())?;
    let reply = String::from_utf8_lossy(&buf[..n]).to_string();

    let recv = server.join().map_err(|_| "server thread panicked".to_string())??;
    if recv != "tcp-ping" || reply != "tcp-pong" {
        return Err(format!("recv={:?} reply={:?}", recv, reply));
    }
    Ok(Outcome::Pass("loopback send/recv ok".into()))
}

// ------------------------------------------------------------------ Tier B
fn b01(_depth: &i32) -> Result<Outcome, String> {
    Ok(Outcome::Skip(
        "std 无 SHA-256（需 sha2 / ring 等 crate）".into(),
    ))
}

fn b02(_depth: &i32) -> Result<Outcome, String> {
    Ok(Outcome::Skip(
        "std 无 Base64（需 base64 crate）".into(),
    ))
}

fn b03(_depth: &i32) -> Result<Outcome, String> {
    Ok(Outcome::Skip(
        "std 无 JSON（需 serde_json 等 crate）".into(),
    ))
}

fn b04(_depth: &i32) -> Result<Outcome, String> {
    Ok(Outcome::Skip(
        "std 无正则（需 regex crate）".into(),
    ))
}

fn b05(_depth: &i32) -> Result<Outcome, String> {
    Ok(Outcome::Skip(
        "std 无 gzip/deflate（需 flate2 crate）".into(),
    ))
}

// -------------------------------------------------------------------- 主流程
fn main() {
    let depth = depth_of();
    if depth > MAX_DEPTH {
        eprintln!("FATAL: probe recursion depth {} exceeded", depth);
        std::process::exit(3);
    }

    // ⚠️ 模式分派必须先于任何检查执行：探针会自我调用，一旦派错就会指数级派生进程。
    let argv: Vec<String> = env::args().skip(1).collect();
    if let Some(first) = argv.first() {
        match first.as_str() {
            "--echo-child" => {
                print!("child-ok\n");
                eprint!("child-err\n");
                let _ = std::io::stdout().flush();
                let _ = std::io::stderr().flush();
                std::process::exit(0);
            }
            "--read-stdin" => {
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
                let t = line.trim();
                if t == "ping" {
                    print!("pong\n");
                } else {
                    print!("unexpected:{}\n", t);
                }
                let _ = std::io::stdout().flush();
                std::process::exit(0);
            }
            "--argv-probe" => {
                let rest = &argv[1..];
                print!("argv-probe:{}:{}\n", rest.len(), rest.join(":"));
                let _ = std::io::stdout().flush();
                std::process::exit(0);
            }
            _ => {}
        }
    }

    let ids = [
        "A01_argv", "A02_env", "A03_streams", "A04_stdin", "A05_file_rw", "A06_file_append",
        "A07_file_binary", "A08_file_stat", "A09_dir_ops", "A10_temp_file", "A11_unicode",
        "A12_large_io", "A13_time", "A14_random", "A15_container", "A16_error",
        "A17_subprocess", "A18_concurrency", "A19_tcp_loopback",
        "B01_sha256", "B02_base64", "B03_json", "B04_regex", "B05_gzip",
    ];
    let checks: [Check; 24] = [
        a01, a02, a03, a04, a05, a06, a07, a08, a09, a10, a11, a12, a13, a14, a15, a16, a17, a18, a19,
        b01, b02, b03, b04, b05,
    ];

    let mut report = Report { pass: 0, fail: 0, skip: 0 };
    for (i, chk) in checks.iter().enumerate() {
        match chk(&depth) {
            Ok(Outcome::Pass(d)) => report.record(ids[i], "PASS", &d),
            Ok(Outcome::Skip(d)) => report.record(ids[i], "SKIP", &d),
            Err(e) => report.record(ids[i], "FAIL", &e),
        }
    }

    println!("SUMMARY rust {} {} {}", report.pass, report.fail, report.skip);
    let _ = std::io::stdout().flush();
    if report.fail > 0 {
        std::process::exit(1);
    }
}
