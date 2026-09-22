// demos 能力探针 —— C# / .NET 实现。协议见 demos/SPEC.md。
//
// 模式（照抄 SPEC §4，跨语言必须同名）：
//   默认              跑全部检查
//   --echo-child      往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
//   --read-stdin      读一行 stdin，内容为 ping 时回 pong
//   --argv-probe A B  回显 argv（argv-probe:2:A:B）
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Net;
using System.Net.Sockets;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;

internal static class Probe
{
    private const string UnicodeS = "中文-日本語-한국어-🚀";
    private const int UnicodeCodepoints = 12;
    private const int UnicodeBytes = 31;
    private const int LargeBytes = 262144;
    private const string Sha256Abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    private const int MaxDepth = 3;

    private static int _depth;
    private static int _pass;
    private static int _fail;
    private static int _skip;

    // A18/A19 的共享计数器。C# 不允许对「被 lambda 捕获的局部变量」取 ref，
    // 所以必须用静态字段。
    private static int _acc;

    private static void Record(string cid, string status, string detail)
    {
        string clean = (detail ?? string.Empty).Replace('\t', ' ').Replace('\r', ' ').Replace('\n', ' ');
        if (clean.Length > 160) clean = clean.Substring(0, 160);
        if (status == "PASS") _pass++;
        else if (status == "FAIL") _fail++;
        else _skip++;
        // 显式写 "\n"：WriteLine 在 Windows 发 \r\n，会把 \r 带进 runner 的 TSV。
        Console.Out.Write("CAP " + cid + " " + status + " " + clean + "\n");
        Console.Out.Flush();
    }

    private static void Run(string cid, Func<string> fn)
    {
        try
        {
            string detail = fn();
            Record(cid, "PASS", detail ?? string.Empty);
        }
        catch (Exception ex)
        {
            Record(cid, "FAIL", ex.GetType().Name + ": " + ex.Message);
        }
    }

    // ------------------------------------------------------------- 自我调用
    // 由 apphost 启动时 ProcessPath 就是本程序；由 `dotnet x.dll` 启动时它是 dotnet 宿主，
    // 此时要把 dll 路径补进参数。
    private static (string exe, List<string> prefix) SelfCommand()
    {
        string proc = Environment.ProcessPath ?? "dotnet";
        string leaf = Path.GetFileNameWithoutExtension(proc).ToLowerInvariant();
        string dll = Assembly.GetEntryAssembly()?.Location ?? string.Empty;
        if (leaf == "dotnet" && dll.Length > 0) return (proc, new List<string> { dll });
        return (proc, new List<string>());
    }

    private static (int code, string stdout, string stderr) Spawn(List<string> args, string stdin = null)
    {
        var (exe, prefix) = SelfCommand();
        var psi = new ProcessStartInfo(exe)
        {
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            RedirectStandardInput = stdin != null,
        };
        foreach (string a in prefix) psi.ArgumentList.Add(a);
        foreach (string a in args) psi.ArgumentList.Add(a);
        psi.Environment["DEMO_DEPTH"] = (_depth + 1).ToString(CultureInfo.InvariantCulture);

        using (Process p = Process.Start(psi))
        {
            if (stdin != null)
            {
                p.StandardInput.Write(stdin);
                p.StandardInput.Close();
            }
            string so = p.StandardOutput.ReadToEnd();
            string se = p.StandardError.ReadToEnd();
            if (!p.WaitForExit(60000))
            {
                try { p.Kill(true); } catch (Exception) { /* 已退出 */ }
                throw new Exception("child timed out");
            }
            return (p.ExitCode, so, se);
        }
    }

    // ------------------------------------------------------------------ Tier A
    private static string A01()
    {
        var (_, so, _) = Spawn(new List<string> { "--argv-probe", "alpha", "beta" });
        string got = so.Trim();
        if (got != "argv-probe:2:alpha:beta") throw new Exception("argv 回显不符: " + got);
        return "2 args round-tripped";
    }

    private static string A02()
    {
        string v = Environment.GetEnvironmentVariable("DEMO_LANG_TAG") ?? string.Empty;
        if (v != "demos-capability") throw new Exception("DEMO_LANG_TAG=" + v);
        return "env visible";
    }

    private static string A03()
    {
        var (_, so, se) = Spawn(new List<string> { "--echo-child" });
        if (so.Trim() != "child-ok") throw new Exception("stdout=" + so.Trim());
        if (!se.Contains("child-err")) throw new Exception("stderr=" + se.Trim());
        if (so.Contains("child-err")) throw new Exception("stderr leaked into stdout");
        return "stdout/stderr separated";
    }

    private static string A04()
    {
        var (_, so, _) = Spawn(new List<string> { "--read-stdin" }, "ping\n");
        if (so.Trim() != "pong") throw new Exception("reply=" + so.Trim());
        return "ping->pong";
    }

    private static string A05()
    {
        File.WriteAllText("a05.txt", "hello-io", new UTF8Encoding(false));
        string got = File.ReadAllText("a05.txt", Encoding.UTF8);
        if (got != "hello-io") throw new Exception("read back " + got);
        return "text round-trip ok";
    }

    private static string A06()
    {
        File.WriteAllText("a06.txt", "a", new UTF8Encoding(false));
        File.AppendAllText("a06.txt", "b", new UTF8Encoding(false));
        string got = File.ReadAllText("a06.txt", Encoding.UTF8);
        if (got != "ab") throw new Exception("append 结果 " + got);
        return "append ok";
    }

    private static string A07()
    {
        byte[] payload = new byte[256];
        for (int i = 0; i < 256; i++) payload[i] = (byte)i;
        File.WriteAllBytes("a07.bin", payload);
        byte[] got = File.ReadAllBytes("a07.bin");
        if (got.Length != payload.Length) throw new Exception("len=" + got.Length);
        for (int i = 0; i < payload.Length; i++)
            if (got[i] != payload[i]) throw new Exception("byte " + i + " differs");
        return "256 bytes incl 0x00/0xFF";
    }

    private static string A08()
    {
        File.WriteAllText("a08.txt", "statted", new UTF8Encoding(false));
        if (!File.Exists("a08.txt")) throw new Exception("file not created");
        long size = new FileInfo("a08.txt").Length;
        if (size != 7) throw new Exception("size=" + size);
        File.Delete("a08.txt");
        if (File.Exists("a08.txt")) throw new Exception("delete failed");
        return "size=7 then removed";
    }

    private static string A09()
    {
        string dir = "a09dir";
        if (Directory.Exists(dir)) Directory.Delete(dir, true);
        Directory.CreateDirectory(dir);
        File.WriteAllText(Path.Combine(dir, "inner.txt"), "x", new UTF8Encoding(false));
        string[] entries = Directory.GetFiles(dir);
        bool found = false;
        foreach (string e in entries)
            if (Path.GetFileName(e) == "inner.txt") found = true;
        if (!found) throw new Exception("listing=" + string.Join(",", entries));
        File.Delete(Path.Combine(dir, "inner.txt"));
        Directory.Delete(dir);
        if (Directory.Exists(dir)) throw new Exception("rmdir failed");
        return "mkdir/list/rmdir ok";
    }

    private static string A10()
    {
        string path = Path.Combine(".", "demos-" + Path.GetRandomFileName() + ".tmp");
        File.WriteAllText(path, "temp-content", new UTF8Encoding(false));
        try
        {
            string got = File.ReadAllText(path, Encoding.UTF8);
            if (got != "temp-content") throw new Exception("temp read " + got);
        }
        finally
        {
            if (File.Exists(path)) File.Delete(path);
        }
        return "unique temp file";
    }

    private static string A11()
    {
        int cps = 0;
        foreach (System.Text.Rune _ in UnicodeS.EnumerateRunes()) cps++;
        if (cps != UnicodeCodepoints) throw new Exception("codepoints=" + cps);
        int nbytes = Encoding.UTF8.GetByteCount(UnicodeS);
        if (nbytes != UnicodeBytes) throw new Exception("bytes=" + nbytes);
        File.WriteAllText("a11.txt", UnicodeS, new UTF8Encoding(false));
        string got = File.ReadAllText("a11.txt", Encoding.UTF8);
        if (got != UnicodeS) throw new Exception("round-trip mismatch");
        return UnicodeCodepoints + " codepoints / " + UnicodeBytes + " bytes";
    }

    private static string A12()
    {
        var sb = new StringBuilder();
        for (int i = 0; i < LargeBytes / 8; i++) sb.Append("abcdefgh");
        byte[] payload = Encoding.ASCII.GetBytes(sb.ToString());
        if (payload.Length != LargeBytes) throw new Exception("payload len=" + payload.Length);
        File.WriteAllBytes("a12.bin", payload);
        byte[] got = File.ReadAllBytes("a12.bin");
        if (got.Length != LargeBytes) throw new Exception("len=" + got.Length);
        for (int i = 0; i < LargeBytes; i++)
            if (got[i] != payload[i]) throw new Exception("byte " + i + " differs");
        return LargeBytes + " bytes ok";
    }

    private static string A13()
    {
        long ms = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        if (ms < 1577836800000L) throw new Exception("epoch=" + ms);
        var sw = Stopwatch.StartNew();
        Thread.Sleep(50);
        double delta = sw.Elapsed.TotalMilliseconds;
        if (delta < 40) throw new Exception("sleep 只测到 " + delta.ToString("F1", CultureInfo.InvariantCulture) + " ms");
        return "sleep " + delta.ToString("F0", CultureInfo.InvariantCulture) + "ms";
    }

    private static string A14()
    {
        int v = new Random().Next(0, 1000);
        if (v < 0 || v >= 1000) throw new Exception("out of range " + v);
        return "in [0,1000)";
    }

    private static string A15()
    {
        var data = new List<int> { 5, 3, 9, 1, 7, 3 };
        data.Sort();
        if (string.Join(",", data) != "1,3,3,5,7,9") throw new Exception("sorted=" + string.Join(",", data));
        var m = new Dictionary<string, int> { { "a", 1 } };
        m["b"] = 2;
        if (!m.TryGetValue("a", out int va) || va != 1 || m["b"] != 2 || m.Count != 2)
            throw new Exception("map broken");
        return "sort + map ok";
    }

    private static string A16()
    {
        bool caught = false;
        try
        {
            File.ReadAllText("definitely-missing-file-xyz");
        }
        catch (IOException)
        {
            caught = true;
        }
        if (!caught) throw new Exception("no error raised for missing file");
        return "IOException caught";
    }

    private static string A17()
    {
        var (code, so, _) = Spawn(new List<string> { "--echo-child" });
        if (code != 0) throw new Exception("child exit=" + code);
        if (so.Trim() != "child-ok") throw new Exception("child stdout=" + so.Trim());
        return "child exit=0, stdout captured";
    }

    private static string A18()
    {
        _acc = 0;
        Task t1 = Task.Run(() => { for (int i = 0; i < 1000; i++) Interlocked.Increment(ref _acc); });
        Task t2 = Task.Run(() => { for (int i = 0; i < 1000; i++) Interlocked.Increment(ref _acc); });
        Task.WaitAll(t1, t2);
        if (_acc != 2000) throw new Exception("total=" + _acc);
        return "2 tasks -> 2000";
    }

    private static string A19()
    {
        var listener = new TcpListener(IPAddress.Loopback, 0);
        listener.Start();
        int port = ((IPEndPoint)listener.LocalEndpoint).Port;
        string recv = null;

        Task serve = Task.Run(() =>
        {
            using (TcpClient c = listener.AcceptTcpClient())
            using (NetworkStream ns = c.GetStream())
            {
                var buf = new byte[64];
                int n = ns.Read(buf, 0, buf.Length);
                recv = Encoding.ASCII.GetString(buf, 0, n);
                byte[] reply = Encoding.ASCII.GetBytes("tcp-pong");
                ns.Write(reply, 0, reply.Length);
                ns.Flush();
            }
        });

        string got;
        using (var cli = new TcpClient())
        {
            cli.Connect(IPAddress.Loopback, port);
            using (NetworkStream s = cli.GetStream())
            {
                byte[] ping = Encoding.ASCII.GetBytes("tcp-ping");
                s.Write(ping, 0, ping.Length);
                s.Flush();
                var buf = new byte[64];
                int n = s.Read(buf, 0, buf.Length);
                got = Encoding.ASCII.GetString(buf, 0, n);
            }
        }
        if (!serve.Wait(10000)) throw new Exception("server thread did not finish");
        listener.Stop();
        if (recv != "tcp-ping" || got != "tcp-pong")
            throw new Exception("recv=" + recv + " reply=" + got);
        return "loopback send/recv ok";
    }

    // ------------------------------------------------------------------ Tier B
    private static string B01()
    {
        string got = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes("abc"))).ToLowerInvariant();
        if (got != Sha256Abc) throw new Exception("sha256=" + got);
        return "sha256(abc) ok";
    }

    private static string B02()
    {
        string enc = Convert.ToBase64String(Encoding.ASCII.GetBytes("abc"));
        if (enc != "YWJj") throw new Exception("b64=" + enc);
        string dec = Encoding.ASCII.GetString(Convert.FromBase64String(enc));
        if (dec != "abc") throw new Exception("b64 decode mismatch");
        return "encode+decode ok";
    }

    private static string B03()
    {
        var src = new Dictionary<string, string> { { "k", "v" }, { "n", "1" } };
        string text = JsonSerializer.Serialize(src);
        var back = JsonSerializer.Deserialize<Dictionary<string, string>>(text);
        if (back == null || back.Count != 2 || back["k"] != "v" || back["n"] != "1")
            throw new Exception("round-trip " + text);
        return "serialize+parse ok";
    }

    private static string B04()
    {
        if (!Regex.IsMatch("abc-123", "^[a-z]+-[0-9]{3}$")) throw new Exception("positive match failed");
        if (Regex.IsMatch("ABC-123", "^[a-z]+-[0-9]{3}$")) throw new Exception("negative match unexpected");
        return "match+reject ok";
    }

    private static string B05()
    {
        var sb = new StringBuilder();
        for (int i = 0; i < 8; i++) sb.Append("gzip-payload-");
        byte[] payload = Encoding.ASCII.GetBytes(sb.ToString());

        byte[] packed;
        using (var ms = new MemoryStream())
        {
            using (var gz = new GZipStream(ms, CompressionLevel.Optimal, true))
                gz.Write(payload, 0, payload.Length);
            packed = ms.ToArray();
        }

        byte[] back;
        using (var ms = new MemoryStream(packed))
        using (var gz = new GZipStream(ms, CompressionMode.Decompress))
        using (var outMs = new MemoryStream())
        {
            gz.CopyTo(outMs);
            back = outMs.ToArray();
        }

        if (back.Length != payload.Length) throw new Exception("len=" + back.Length);
        for (int i = 0; i < payload.Length; i++)
            if (back[i] != payload[i]) throw new Exception("mismatch at " + i);
        return "compress+decompress ok";
    }

    // -------------------------------------------------------------------- 主流程
    private static int Main(string[] argv)
    {
        int d;
        _depth = int.TryParse(Environment.GetEnvironmentVariable("DEMO_DEPTH"), NumberStyles.Integer,
                              CultureInfo.InvariantCulture, out d) ? d : 0;
        if (_depth > MaxDepth)
        {
            Console.Error.Write("FATAL: probe recursion depth " + _depth + " exceeded\n");
            return 3;
        }

        // ⚠️ 模式分派必须先于任何检查执行：探针会自我调用，一旦派错就会指数级派生进程。
        if (argv.Length >= 1)
        {
            switch (argv[0])
            {
                case "--echo-child":
                    Console.Out.Write("child-ok\n");
                    Console.Error.Write("child-err\n");
                    Console.Out.Flush();
                    Console.Error.Flush();
                    return 0;

                case "--read-stdin":
                {
                    string line = (Console.In.ReadLine() ?? string.Empty).Trim();
                    Console.Out.Write(line == "ping" ? "pong\n" : "unexpected:" + line + "\n");
                    Console.Out.Flush();
                    return 0;
                }

                case "--argv-probe":
                {
                    var rest = new List<string>();
                    for (int i = 1; i < argv.Length; i++) rest.Add(argv[i]);
                    Console.Out.Write("argv-probe:" + rest.Count + ":" + string.Join(":", rest) + "\n");
                    Console.Out.Flush();
                    return 0;
                }
            }
        }

        string[] ids =
        {
            "A01_argv", "A02_env", "A03_streams", "A04_stdin", "A05_file_rw", "A06_file_append",
            "A07_file_binary", "A08_file_stat", "A09_dir_ops", "A10_temp_file", "A11_unicode",
            "A12_large_io", "A13_time", "A14_random", "A15_container", "A16_error",
            "A17_subprocess", "A18_concurrency", "A19_tcp_loopback",
            "B01_sha256", "B02_base64", "B03_json", "B04_regex", "B05_gzip"
        };
        Func<string>[] fns =
        {
            A01, A02, A03, A04, A05, A06, A07, A08, A09, A10, A11, A12, A13, A14, A15, A16, A17, A18, A19,
            B01, B02, B03, B04, B05
        };

        for (int i = 0; i < fns.Length; i++) Run(ids[i], fns[i]);

        Console.Out.Write("SUMMARY csharp " + _pass + " " + _fail + " " + _skip + "\n");
        Console.Out.Flush();
        return _fail > 0 ? 1 : 0;
    }
}
