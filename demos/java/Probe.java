// demos 能力探针 —— Java 实现。协议见 demos/SPEC.md。
//
// 模式（照抄 SPEC §4，跨语言必须同名）：
//   默认              跑全部检查
//   --echo-child      往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
//   --read-stdin      读一行 stdin，内容为 ping 时回 pong
//   --argv-probe A B  回显 argv（argv-probe:2:A:B）
import java.io.BufferedReader;
import java.io.File;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.PrintStream;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.regex.Pattern;
import java.util.zip.GZIPInputStream;
import java.util.zip.GZIPOutputStream;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;

public final class Probe {

    private static final String UNICODE_S = "中文-日本語-한국어-🚀";
    private static final int UNICODE_CODEPOINTS = 12;
    private static final int UNICODE_BYTES = 31;
    private static final int LARGE_BYTES = 262144;
    private static final String SHA256_ABC =
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    private static final int MAX_DEPTH = 3;

    private static int depth;
    private static int npass;
    private static int nfail;
    private static int nskip;

    private interface Check {
        String run() throws Exception;
    }

    private static final class SkipCheck extends Exception {
        private static final long serialVersionUID = 1L;

        SkipCheck(String reason) {
            super(reason);
        }
    }

    private static void record(String cid, String status, String detail) {
        String clean = detail == null ? "" : detail.replace('\t', ' ').replace('\r', ' ').replace('\n', ' ');
        if (clean.length() > 160) {
            clean = clean.substring(0, 160);
        }
        if ("PASS".equals(status)) {
            npass++;
        } else if ("FAIL".equals(status)) {
            nfail++;
        } else {
            nskip++;
        }
        System.out.print("CAP " + cid + " " + status + " " + clean + "\n");
        System.out.flush();
    }

    private static void run(String cid, Check c) {
        try {
            record(cid, "PASS", c.run());
        } catch (SkipCheck e) {
            record(cid, "SKIP", e.getMessage());
        } catch (Exception e) {
            record(cid, "FAIL", e.getClass().getSimpleName() + ": " + e.getMessage());
        }
    }

    // ------------------------------------------------------------- 自我调用
    private static String javaExe() {
        String home = System.getProperty("java.home");
        File f = new File(home, "bin/java");
        if (!f.exists()) {
            f = new File(home, "bin/java.exe");
        }
        return f.exists() ? f.getAbsolutePath() : "java";
    }

    /** 本类被加载的路径（即 javac -d 的输出目录）。子进程用同一个 classpath 启动。 */
    private static String selfClasspath() {
        try {
            URL u = Probe.class.getProtectionDomain().getCodeSource().getLocation();
            return Paths.get(u.toURI()).toString();
        } catch (Exception e) {
            return ".";
        }
    }

    private static final class Result {
        final int code;
        final String out;
        final String err;

        Result(int code, String out, String err) {
            this.code = code;
            this.out = out;
            this.err = err;
        }
    }

    private static Result spawn(List<String> args, String stdinData) throws Exception {
        List<String> cmd = new ArrayList<>();
        cmd.add(javaExe());
        cmd.add("-Dfile.encoding=UTF-8");
        cmd.add("-cp");
        cmd.add(selfClasspath());
        cmd.add("Probe");
        cmd.addAll(args);

        ProcessBuilder pb = new ProcessBuilder(cmd);
        pb.environment().put("DEMO_DEPTH", String.valueOf(depth + 1));
        Process p = pb.start();
        if (stdinData != null) {
            OutputStream os = p.getOutputStream();
            os.write(stdinData.getBytes(StandardCharsets.UTF_8));
            os.flush();
            os.close();
        } else {
            p.getOutputStream().close();
        }

        StringBuilder so = new StringBuilder();
        StringBuilder se = new StringBuilder();
        Thread t1 = new Thread(() -> drain(p.getInputStream(), so));
        Thread t2 = new Thread(() -> drain(p.getErrorStream(), se));
        t1.start();
        t2.start();
        if (!p.waitFor(60, java.util.concurrent.TimeUnit.SECONDS)) {
            p.destroyForcibly();
            throw new IOException("child timed out");
        }
        t1.join(5000);
        t2.join(5000);
        return new Result(p.exitValue(), so.toString(), se.toString());
    }

    private static void drain(java.io.InputStream in, StringBuilder sb) {
        try (BufferedReader r = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8))) {
            String line;
            while ((line = r.readLine()) != null) {
                sb.append(line).append('\n');
            }
        } catch (IOException e) {
            // 子进程退出时管道可能被关闭，属正常
        }
    }

    // ------------------------------------------------------------------ Tier A
    private static String a01() throws Exception {
        List<String> a = new ArrayList<>();
        a.add("--argv-probe");
        a.add("alpha");
        a.add("beta");
        Result r = spawn(a, null);
        String got = r.out.trim();
        if (!"argv-probe:2:alpha:beta".equals(got)) {
            throw new AssertionError("argv 回显不符: " + got);
        }
        return "2 args round-tripped";
    }

    private static String a02() {
        String v = System.getenv("DEMO_LANG_TAG");
        if (!"demos-capability".equals(v)) {
            throw new AssertionError("DEMO_LANG_TAG=" + v);
        }
        return "env visible";
    }

    private static String a03() throws Exception {
        List<String> a = new ArrayList<>();
        a.add("--echo-child");
        Result r = spawn(a, null);
        if (!"child-ok".equals(r.out.trim())) {
            throw new AssertionError("stdout=" + r.out.trim());
        }
        if (!r.err.contains("child-err")) {
            throw new AssertionError("stderr=" + r.err.trim());
        }
        if (r.out.contains("child-err")) {
            throw new AssertionError("stderr leaked into stdout");
        }
        return "stdout/stderr separated";
    }

    private static String a04() throws Exception {
        List<String> a = new ArrayList<>();
        a.add("--read-stdin");
        Result r = spawn(a, "ping\n");
        if (!"pong".equals(r.out.trim())) {
            throw new AssertionError("reply=" + r.out.trim());
        }
        return "ping->pong";
    }

    private static String a05() throws Exception {
        Files.write(Paths.get("a05.txt"), "hello-io".getBytes(StandardCharsets.UTF_8));
        String got = new String(Files.readAllBytes(Paths.get("a05.txt")), StandardCharsets.UTF_8);
        if (!"hello-io".equals(got)) {
            throw new AssertionError("read back " + got);
        }
        return "text round-trip ok";
    }

    private static String a06() throws Exception {
        Path p = Paths.get("a06.txt");
        Files.write(p, "a".getBytes(StandardCharsets.UTF_8));
        java.nio.file.StandardOpenOption app = java.nio.file.StandardOpenOption.APPEND;
        Files.write(p, "b".getBytes(StandardCharsets.UTF_8), app);
        String got = new String(Files.readAllBytes(p), StandardCharsets.UTF_8);
        if (!"ab".equals(got)) {
            throw new AssertionError("append 结果 " + got);
        }
        return "append ok";
    }

    private static String a07() throws Exception {
        byte[] payload = new byte[256];
        for (int i = 0; i < 256; i++) {
            payload[i] = (byte) i;
        }
        Files.write(Paths.get("a07.bin"), payload);
        byte[] got = Files.readAllBytes(Paths.get("a07.bin"));
        if (!Arrays.equals(payload, got)) {
            throw new AssertionError("二进制往返不一致 len=" + got.length);
        }
        return "256 bytes incl 0x00/0xFF";
    }

    private static String a08() throws Exception {
        Path p = Paths.get("a08.txt");
        Files.write(p, "statted".getBytes(StandardCharsets.UTF_8));
        if (!Files.exists(p)) {
            throw new AssertionError("file not created");
        }
        long size = Files.size(p);
        if (size != 7) {
            throw new AssertionError("size=" + size);
        }
        Files.delete(p);
        if (Files.exists(p)) {
            throw new AssertionError("delete failed");
        }
        return "size=7 then removed";
    }

    private static String a09() throws Exception {
        Path d = Paths.get("a09dir");
        if (Files.exists(d)) {
            Files.delete(d.resolve("inner.txt"));
            Files.delete(d);
        }
        Files.createDirectory(d);
        Files.write(d.resolve("inner.txt"), "x".getBytes(StandardCharsets.UTF_8));
        boolean found = false;
        File[] entries = d.toFile().listFiles();
        if (entries == null) {
            throw new AssertionError("listFiles returned null");
        }
        for (File e : entries) {
            if ("inner.txt".equals(e.getName())) {
                found = true;
            }
        }
        if (!found) {
            throw new AssertionError("listing missing inner.txt");
        }
        Files.delete(d.resolve("inner.txt"));
        Files.delete(d);
        if (Files.exists(d)) {
            throw new AssertionError("rmdir failed");
        }
        return "mkdir/list/rmdir ok";
    }

    private static String a10() throws Exception {
        Path p = Files.createTempFile(Paths.get("."), "demos-", ".tmp");
        try {
            Files.write(p, "temp-content".getBytes(StandardCharsets.UTF_8));
            String got = new String(Files.readAllBytes(p), StandardCharsets.UTF_8);
            if (!"temp-content".equals(got)) {
                throw new AssertionError("temp read " + got);
            }
        } finally {
            Files.deleteIfExists(p);
        }
        return "unique temp file";
    }

    private static String a11() throws Exception {
        int cps = UNICODE_S.codePointCount(0, UNICODE_S.length());
        if (cps != UNICODE_CODEPOINTS) {
            throw new AssertionError("codepoints=" + cps);
        }
        byte[] raw = UNICODE_S.getBytes(StandardCharsets.UTF_8);
        if (raw.length != UNICODE_BYTES) {
            throw new AssertionError("bytes=" + raw.length);
        }
        Path p = Paths.get("a11.txt");
        Files.write(p, raw);
        String got = new String(Files.readAllBytes(p), StandardCharsets.UTF_8);
        if (!UNICODE_S.equals(got)) {
            throw new AssertionError("round-trip mismatch");
        }
        return UNICODE_CODEPOINTS + " codepoints / " + UNICODE_BYTES + " bytes";
    }

    private static String a12() throws Exception {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < LARGE_BYTES / 8; i++) {
            sb.append("abcdefgh");
        }
        byte[] payload = sb.toString().getBytes(StandardCharsets.US_ASCII);
        if (payload.length != LARGE_BYTES) {
            throw new AssertionError("payload len=" + payload.length);
        }
        Files.write(Paths.get("a12.bin"), payload);
        byte[] got = Files.readAllBytes(Paths.get("a12.bin"));
        if (!Arrays.equals(payload, got)) {
            throw new AssertionError("len=" + got.length);
        }
        return LARGE_BYTES + " bytes ok";
    }

    private static String a13() throws Exception {
        long ms = System.currentTimeMillis();
        if (ms < 1577836800000L) {
            throw new AssertionError("epoch=" + ms);
        }
        long t0 = System.nanoTime();
        Thread.sleep(50);
        double delta = (System.nanoTime() - t0) / 1e6;
        if (delta < 40) {
            throw new AssertionError(String.format("sleep 只测到 %.1f ms", delta));
        }
        return String.format("sleep %.0fms", delta);
    }

    private static String a14() {
        int v = new java.util.Random().nextInt(1000);
        if (v < 0 || v >= 1000) {
            throw new AssertionError("out of range " + v);
        }
        return "in [0,1000)";
    }

    private static String a15() {
        int[] data = {5, 3, 9, 1, 7, 3};
        Arrays.sort(data);
        if (!Arrays.toString(data).equals("[1, 3, 3, 5, 7, 9]")) {
            throw new AssertionError("sorted=" + Arrays.toString(data));
        }
        Map<String, Integer> m = new HashMap<>();
        m.put("a", 1);
        m.put("b", 2);
        if (m.get("a") != 1 || m.get("b") != 2 || m.size() != 2) {
            throw new AssertionError("map broken");
        }
        return "sort + map ok";
    }

    private static String a16() {
        boolean caught = false;
        try {
            Files.readAllBytes(Paths.get("definitely-missing-file-xyz"));
        } catch (IOException e) {
            caught = true;
        }
        if (!caught) {
            throw new AssertionError("no error raised for missing file");
        }
        return "IOException caught";
    }

    private static String a17() throws Exception {
        List<String> a = new ArrayList<>();
        a.add("--echo-child");
        Result r = spawn(a, null);
        if (r.code != 0) {
            throw new AssertionError("child exit=" + r.code);
        }
        if (!"child-ok".equals(r.out.trim())) {
            throw new AssertionError("child stdout=" + r.out.trim());
        }
        return "child exit=0, stdout captured";
    }

    private static String a18() throws Exception {
        AtomicInteger total = new AtomicInteger(0);
        Thread t1 = new Thread(() -> {
            for (int i = 0; i < 1000; i++) {
                total.incrementAndGet();
            }
        });
        Thread t2 = new Thread(() -> {
            for (int i = 0; i < 1000; i++) {
                total.incrementAndGet();
            }
        });
        t1.start();
        t2.start();
        t1.join();
        t2.join();
        if (total.get() != 2000) {
            throw new AssertionError("total=" + total.get());
        }
        return "2 threads -> 2000";
    }

    private static String a19() throws Exception {
        InetAddress loop = InetAddress.getByName("127.0.0.1");
        final ServerSocket srv = new ServerSocket(0, 4, loop);
        final int port = srv.getLocalPort();
        final String[] recv = new String[1];

        Thread t = new Thread(() -> {
            try (Socket conn = srv.accept()) {
                byte[] buf = new byte[64];
                int n = conn.getInputStream().read(buf);
                recv[0] = new String(buf, 0, n, StandardCharsets.US_ASCII);
                conn.getOutputStream().write("tcp-pong".getBytes(StandardCharsets.US_ASCII));
                conn.getOutputStream().flush();
            } catch (IOException e) {
                recv[0] = "ERR:" + e.getMessage();
            }
        });
        t.start();

        String got;
        try (Socket cli = new Socket(loop, port)) {
            cli.getOutputStream().write("tcp-ping".getBytes(StandardCharsets.US_ASCII));
            cli.getOutputStream().flush();
            byte[] buf = new byte[64];
            int n = cli.getInputStream().read(buf);
            got = new String(buf, 0, n, StandardCharsets.US_ASCII);
        }
        t.join(10000);
        srv.close();
        if (!"tcp-ping".equals(recv[0]) || !"tcp-pong".equals(got)) {
            throw new AssertionError("recv=" + recv[0] + " reply=" + got);
        }
        return "loopback send/recv ok";
    }

    // ------------------------------------------------------------------ Tier B
    private static String b01() throws Exception {
        byte[] d = MessageDigest.getInstance("SHA-256").digest("abc".getBytes(StandardCharsets.UTF_8));
        StringBuilder sb = new StringBuilder();
        for (byte b : d) {
            sb.append(String.format("%02x", b));
        }
        if (!SHA256_ABC.equals(sb.toString())) {
            throw new AssertionError("sha256=" + sb);
        }
        return "sha256(abc) ok";
    }

    private static String b02() {
        String enc = Base64.getEncoder().encodeToString("abc".getBytes(StandardCharsets.US_ASCII));
        if (!"YWJj".equals(enc)) {
            throw new AssertionError("b64=" + enc);
        }
        String dec = new String(Base64.getDecoder().decode(enc), StandardCharsets.US_ASCII);
        if (!"abc".equals(dec)) {
            throw new AssertionError("b64 decode mismatch");
        }
        return "encode+decode ok";
    }

    private static String b03() throws Exception {
        // JDK 标准库**没有** JSON：java.util 与 java.text 都不含序列化器。
        throw new SkipCheck("JDK 标准库无 JSON（需 Gson/Jackson 等第三方库）");
    }

    private static String b04() {
        Pattern p = Pattern.compile("^[a-z]+-[0-9]{3}$");
        if (!p.matcher("abc-123").matches()) {
            throw new AssertionError("positive match failed");
        }
        if (p.matcher("ABC-123").matches()) {
            throw new AssertionError("negative match unexpected");
        }
        return "match+reject ok";
    }

    private static String b05() throws Exception {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < 8; i++) {
            sb.append("gzip-payload-");
        }
        byte[] payload = sb.toString().getBytes(StandardCharsets.US_ASCII);

        ByteArrayOutputStream bos = new ByteArrayOutputStream();
        try (GZIPOutputStream gz = new GZIPOutputStream(bos)) {
            gz.write(payload);
        }
        byte[] packed = bos.toByteArray();

        ByteArrayOutputStream back = new ByteArrayOutputStream();
        try (GZIPInputStream gz = new GZIPInputStream(new ByteArrayInputStream(packed))) {
            byte[] buf = new byte[512];
            int n;
            while ((n = gz.read(buf)) > 0) {
                back.write(buf, 0, n);
            }
        }
        if (!Arrays.equals(payload, back.toByteArray())) {
            throw new AssertionError("gzip round-trip mismatch");
        }
        return "compress+decompress ok";
    }

    // -------------------------------------------------------------------- 主流程
    public static void main(String[] argv) throws Exception {
        // ⚠️ 先把 stdout/stderr 换成显式 UTF-8 的 PrintStream。
        // JEP 400 之后 `-Dfile.encoding` 只影响 Charset.defaultCharset()，**管不到**
        // System.out：重定向到文件时它用的是原生编码（本机 CP936/GBK），
        // 于是中文 detail 被写成 GBK 字节 ⇒ 输出文件不是合法 UTF-8 ⇒
        // 下游 grep 判成 "binary file" 并丢掉整行（实测丢掉了带中文的 B03）。
        // 这里直接按字节钉死 UTF-8，彻底不依赖平台编码。
        System.setOut(new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8));
        System.setErr(new PrintStream(new FileOutputStream(FileDescriptor.err), true, StandardCharsets.UTF_8));

        String envDepth = System.getenv("DEMO_DEPTH");
        depth = envDepth == null ? 0 : Integer.parseInt(envDepth.trim());
        if (depth > MAX_DEPTH) {
            System.err.print("FATAL: probe recursion depth " + depth + " exceeded\n");
            System.exit(3);
        }

        // ⚠️ 模式分派必须先于任何检查执行：探针会自我调用，一旦派错就会指数级派生进程。
        if (argv.length >= 1) {
            if ("--echo-child".equals(argv[0])) {
                System.out.print("child-ok\n");
                System.err.print("child-err\n");
                System.out.flush();
                System.err.flush();
                System.exit(0);
            }
            if ("--read-stdin".equals(argv[0])) {
                BufferedReader r = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
                String line = r.readLine();
                line = line == null ? "" : line.trim();
                System.out.print("ping".equals(line) ? "pong\n" : "unexpected:" + line + "\n");
                System.out.flush();
                System.exit(0);
            }
            if ("--argv-probe".equals(argv[0])) {
                List<String> rest = new ArrayList<>();
                for (int i = 1; i < argv.length; i++) {
                    rest.add(argv[i]);
                }
                System.out.print("argv-probe:" + rest.size() + ":" + String.join(":", rest) + "\n");
                System.out.flush();
                System.exit(0);
            }
        }

        String[] ids = {
            "A01_argv", "A02_env", "A03_streams", "A04_stdin", "A05_file_rw", "A06_file_append",
            "A07_file_binary", "A08_file_stat", "A09_dir_ops", "A10_temp_file", "A11_unicode",
            "A12_large_io", "A13_time", "A14_random", "A15_container", "A16_error",
            "A17_subprocess", "A18_concurrency", "A19_tcp_loopback",
            "B01_sha256", "B02_base64", "B03_json", "B04_regex", "B05_gzip"
        };
        Check[] fns = {
            Probe::a01, Probe::a02, Probe::a03, Probe::a04, Probe::a05, Probe::a06,
            Probe::a07, Probe::a08, Probe::a09, Probe::a10, Probe::a11, Probe::a12,
            Probe::a13, Probe::a14, Probe::a15, Probe::a16, Probe::a17, Probe::a18, Probe::a19,
            Probe::b01, Probe::b02, Probe::b03, Probe::b04, Probe::b05
        };

        for (int i = 0; i < fns.length; i++) {
            run(ids[i], fns[i]);
        }

        System.out.print("SUMMARY java " + npass + " " + nfail + " " + nskip + "\n");
        System.out.flush();
        System.exit(nfail > 0 ? 1 : 0);
    }
}
