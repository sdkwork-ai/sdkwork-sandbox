<?php
// demos 能力探针 —— PHP 实现。协议见 demos/SPEC.md。
//
// 模式（SPEC §4 的四个 + 一个 PHP 专用）：
//   默认                    跑全部检查
//   --echo-child            往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
//   --read-stdin            读一行 stdin，内容为 ping 时回 pong
//   --argv-probe A B        回显 argv（argv-probe:2:A:B）
//   --count-worker <file>   往文件原子追加 1000 行（A18 并发用；PHP CLI 无线程，
//                           只能用「多进程 + flock 互斥 + 汇总」来演示真正的并发）
//
// ⚠️ PHP 的 CLI 构建里没有线程（pcntl 在 Windows 上也没有），所以 A18 用
//    两个并发子进程 + 文件锁累加来验证「并发执行体 + join + 结果正确」。

const UNICODE_S = "中文-日本語-한국어-🚀";
const UNICODE_CODEPOINTS = 12;
const UNICODE_BYTES = 31;
const LARGE_BYTES = 262144;
const SHA256_ABC = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
// 递归护栏：探针自我调用，模式分派写错就会指数级派生进程。
const MAX_DEPTH = 3;

$GLOBALS['npass'] = 0;
$GLOBALS['nfail'] = 0;
$GLOBALS['nskip'] = 0;
$GLOBALS['depth'] = 0;

class DemoSkip extends Exception
{
}

function demo_record($cid, $status, $detail = '')
{
    $clean = preg_replace('/[\t\r\n]+/', ' ', (string) $detail);
    if (strlen($clean) > 160) {
        $clean = substr($clean, 0, 160);
    }
    if ($status === 'PASS') {
        $GLOBALS['npass']++;
    } elseif ($status === 'FAIL') {
        $GLOBALS['nfail']++;
    } else {
        $GLOBALS['nskip']++;
    }
    fwrite(STDOUT, "CAP {$cid} {$status} {$clean}\n");
    fflush(STDOUT);
}

function demo_exempt($cid, $reason)
{
    fwrite(STDOUT, "EXEMPT {$cid} {$reason}\n");
    fflush(STDOUT);
}

function demo_run($cid, $fn)
{
    try {
        $detail = $fn();
        demo_record($cid, 'PASS', $detail === null ? '' : $detail);
    } catch (DemoSkip $e) {
        demo_record($cid, 'SKIP', $e->getMessage());
    } catch (Throwable $e) {
        demo_record($cid, 'FAIL', get_class($e) . ': ' . $e->getMessage());
    }
}

function demo_assert($cond, $msg)
{
    if (!$cond) {
        throw new RuntimeException($msg);
    }
}

/**
 * 自我调用。返回 [exitCode, stdout, stderr]。
 * $stdin 非 null 时把它写进子进程 stdin。
 */
function demo_spawn(array $args, $stdin = null)
{
    $desc = [
        0 => ['pipe', 'r'],
        1 => ['pipe', 'w'],
        2 => ['pipe', 'w'],
    ];
    // getenv() 无参形态（PHP 7.1+）拿到完整环境，比读 $_ENV 可靠 ——
    // $_ENV 是否填充取决于 php.ini 的 variables_order。
    $env = getenv();
    if (!is_array($env)) {
        $env = [];
    }
    $env['DEMO_DEPTH'] = (string) ($GLOBALS['depth'] + 1);

    $cmd = array_merge([PHP_BINARY, __FILE__], $args);
    $proc = proc_open($cmd, $desc, $pipes, null, $env);
    if (!is_resource($proc)) {
        throw new RuntimeException('proc_open failed');
    }
    if ($stdin !== null) {
        fwrite($pipes[0], $stdin);
    }
    fclose($pipes[0]);
    $out = stream_get_contents($pipes[1]);
    $err = stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);
    $code = proc_close($proc);
    return [$code, $out, $err];
}

// ------------------------------------------------------------------ Tier A
function demo_a01()
{
    [$code, $out, $err] = demo_spawn(['--argv-probe', 'alpha', 'beta']);
    $got = trim($out);
    demo_assert($got === 'argv-probe:2:alpha:beta', "argv 回显不符: {$got}");
    return '2 args round-tripped';
}

function demo_a02()
{
    $v = getenv('DEMO_LANG_TAG');
    demo_assert($v === 'demos-capability', 'DEMO_LANG_TAG=' . var_export($v, true));
    return 'env visible';
}

function demo_a03()
{
    [$code, $out, $err] = demo_spawn(['--echo-child']);
    demo_assert(trim($out) === 'child-ok', 'stdout=' . trim($out));
    demo_assert(strpos($err, 'child-err') !== false, 'stderr=' . trim($err));
    demo_assert(strpos($out, 'child-err') === false, 'stderr leaked into stdout');
    return 'stdout/stderr separated';
}

function demo_a04()
{
    [$code, $out, $err] = demo_spawn(['--read-stdin'], "ping\n");
    demo_assert(trim($out) === 'pong', 'reply=' . trim($out));
    return 'ping->pong';
}

function demo_a05()
{
    file_put_contents('a05.txt', 'hello-io');
    $got = file_get_contents('a05.txt');
    demo_assert($got === 'hello-io', 'read back ' . var_export($got, true));
    return 'text round-trip ok';
}

function demo_a06()
{
    file_put_contents('a06.txt', 'a');
    file_put_contents('a06.txt', 'b', FILE_APPEND);
    $got = file_get_contents('a06.txt');
    demo_assert($got === 'ab', 'append 结果 ' . var_export($got, true));
    return 'append ok';
}

function demo_a07()
{
    $payload = '';
    for ($i = 0; $i < 256; $i++) {
        $payload .= chr($i);
    }
    file_put_contents('a07.bin', $payload);
    $got = file_get_contents('a07.bin');
    demo_assert($got === $payload, 'binary round-trip len=' . strlen($got));
    return '256 bytes incl 0x00/0xFF';
}

function demo_a08()
{
    file_put_contents('a08.txt', 'statted');
    demo_assert(file_exists('a08.txt'), 'file not created');
    $size = filesize('a08.txt');
    demo_assert($size === 7, "size={$size}");
    unlink('a08.txt');
    clearstatcache();
    demo_assert(!file_exists('a08.txt'), 'unlink failed');
    return 'size=7 then removed';
}

function demo_a09()
{
    $d = 'a09dir';
    if (is_dir($d)) {
        @unlink($d . '/inner.txt');
        @rmdir($d);
    }
    mkdir($d);
    file_put_contents($d . '/inner.txt', 'x');
    $entries = scandir($d);
    demo_assert(in_array('inner.txt', $entries, true), 'listing=' . implode(',', $entries));
    unlink($d . '/inner.txt');
    rmdir($d);
    clearstatcache();
    demo_assert(!is_dir($d), 'rmdir failed');
    return 'mkdir/list/rmdir ok';
}

function demo_a10()
{
    // tempnam('.') 把临时文件建在**当前工作目录**，而不是系统 temp：
    // SPEC 要求临时产物落在 target/demos-work/ 内。
    $path = tempnam('.', 'demos');
    if ($path === false) {
        throw new RuntimeException('tempnam failed');
    }
    try {
        file_put_contents($path, 'temp-content');
        $got = file_get_contents($path);
        demo_assert($got === 'temp-content', 'temp read ' . var_export($got, true));
    } finally {
        if (file_exists($path)) {
            unlink($path);
        }
    }
    return 'unique temp file';
}

function demo_a11()
{
    // 不用 mb_* —— php-mbstring 在某些发行版是独立包，可能没装。
    // PCRE 的 /u 修饰符足够数码点。
    $n = preg_match_all('/./u', UNICODE_S, $m);
    demo_assert($n === UNICODE_CODEPOINTS, "codepoints={$n}");
    $bytes = strlen(UNICODE_S);
    demo_assert($bytes === UNICODE_BYTES, "bytes={$bytes}");
    file_put_contents('a11.txt', UNICODE_S);
    $got = file_get_contents('a11.txt');
    demo_assert($got === UNICODE_S, 'round-trip mismatch');
    return UNICODE_CODEPOINTS . ' codepoints / ' . UNICODE_BYTES . ' bytes';
}

function demo_a12()
{
    $payload = str_repeat('abcdefgh', LARGE_BYTES / 8);
    demo_assert(strlen($payload) === LARGE_BYTES, 'payload len=' . strlen($payload));
    file_put_contents('a12.bin', $payload);
    $got = file_get_contents('a12.bin');
    demo_assert(strlen($got) === LARGE_BYTES, 'len=' . strlen($got));
    demo_assert($got === $payload, 'content mismatch');
    return LARGE_BYTES . ' bytes ok';
}

function demo_a13()
{
    $ms = (int) round(microtime(true) * 1000);
    demo_assert($ms > 1577836800000, "epoch={$ms}");
    $t0 = hrtime(true);
    usleep(50000);
    $delta = (hrtime(true) - $t0) / 1e6;
    demo_assert($delta >= 40, sprintf('sleep 只测到 %.1f ms', $delta));
    return sprintf('sleep %.0fms', $delta);
}

function demo_a14()
{
    $v = random_int(0, 999);
    demo_assert($v >= 0 && $v < 1000, "out of range {$v}");
    return 'in [0,1000)';
}

function demo_a15()
{
    $data = [5, 3, 9, 1, 7, 3];
    sort($data);
    demo_assert(implode(',', $data) === '1,3,3,5,7,9', 'sorted=' . implode(',', $data));
    $m = ['a' => 1];
    $m['b'] = 2;
    demo_assert(isset($m['a']) && $m['a'] === 1 && $m['b'] === 2 && count($m) === 2, 'map broken');
    return 'sort + map ok';
}

function demo_a16()
{
    // 读不存在的文件，PHP 默认只发 warning 不抛异常；把 warning 提升成异常才算"捕获到错误"。
    $caught = false;
    set_error_handler(function ($no, $str) {
        throw new ErrorException($str, 0, $no);
    });
    try {
        file_get_contents('definitely-missing-file-xyz');
    } catch (Throwable $e) {
        $caught = true;
    } finally {
        restore_error_handler();
    }
    demo_assert($caught, 'no error raised for missing file');
    return 'ErrorException caught';
}

function demo_a17()
{
    [$code, $out, $err] = demo_spawn(['--echo-child']);
    demo_assert($code === 0, "child exit={$code}");
    demo_assert(trim($out) === 'child-ok', 'child stdout=' . trim($out));
    return 'child exit=0, stdout captured';
}

function demo_a18()
{
    // PHP CLI 无线程：用 2 个并发子进程 + flock 互斥 + 汇总来验证并发与 join。
    $file = 'a18-counter.txt';
    if (file_exists($file)) {
        unlink($file);
    }
    touch($file);

    $procs = [];
    $pipesAll = [];
    for ($i = 0; $i < 2; $i++) {
        $desc = [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']];
        $proc = proc_open([PHP_BINARY, __FILE__, '--count-worker', $file], $desc, $pipes);
        if (!is_resource($proc)) {
            throw new RuntimeException('proc_open failed for worker');
        }
        fclose($pipes[0]);
        $procs[] = $proc;
        $pipesAll[] = $pipes;
    }

    $codes = [];
    foreach ($procs as $i => $proc) {
        stream_get_contents($pipesAll[$i][1]);
        $werr = stream_get_contents($pipesAll[$i][2]);
        fclose($pipesAll[$i][1]);
        fclose($pipesAll[$i][2]);
        $codes[] = proc_close($proc);          // join
        if ($codes[$i] !== 0) {
            throw new RuntimeException("worker exit={$codes[$i]} stderr={$werr}");
        }
    }
    $total = count(file($file));
    demo_assert($total === 2000, "总行数={$total}（应为 2000，说明 flock 互斥失效）");
    return '2 concurrent workers -> 2000 (flock)';
}

function demo_a19()
{
    // 单进程即可完成回环测试：listen 之后 connect 会在内核完成三次握手（backlog），
    // 不必先 accept。
    $srv = @stream_socket_server('tcp://127.0.0.1:0', $errno, $errstr);
    if ($srv === false) {
        throw new RuntimeException("stream_socket_server failed: {$errstr}");
    }
    $name = stream_socket_get_name($srv, false);
    $port = (int) substr($name, strrpos($name, ':') + 1);

    $cli = @stream_socket_client("tcp://127.0.0.1:{$port}", $eno, $estr, 5);
    if ($cli === false) {
        fclose($srv);
        throw new RuntimeException("stream_socket_client failed: {$estr}");
    }
    stream_set_timeout($cli, 5);
    fwrite($cli, 'tcp-ping');

    $conn = stream_socket_accept($srv, 5);
    if ($conn === false) {
        fclose($cli);
        fclose($srv);
        throw new RuntimeException('accept timed out');
    }
    stream_set_timeout($conn, 5);
    $recv = fread($conn, 64);
    fwrite($conn, 'tcp-pong');
    fflush($conn);
    $reply = fread($cli, 64);

    fclose($conn);
    fclose($cli);
    fclose($srv);

    demo_assert($recv === 'tcp-ping' && $reply === 'tcp-pong', "recv={$recv} reply={$reply}");
    return 'loopback send/recv ok';
}

// ------------------------------------------------------------------ Tier B
function demo_b01()
{
    demo_assert(function_exists('hash'), 'hash() not available');
    $got = hash('sha256', 'abc');
    demo_assert($got === SHA256_ABC, "sha256={$got}");
    return 'sha256(abc) ok';
}

function demo_b02()
{
    $enc = base64_encode('abc');
    demo_assert($enc === 'YWJj', "b64={$enc}");
    demo_assert(base64_decode($enc) === 'abc', 'b64 decode mismatch');
    return 'encode+decode ok';
}

function demo_b03()
{
    if (!function_exists('json_encode')) {
        throw new DemoSkip('php-json 扩展未启用');
    }
    $obj = ['k' => [1, 2, 3], 'n' => 'v'];
    $text = json_encode($obj);
    $back = json_decode($text, true);
    demo_assert($back == $obj, 'round-trip ' . var_export($back, true));
    return 'serialize+parse ok';
}

function demo_b04()
{
    demo_assert(preg_match('/^[a-z]+-[0-9]{3}$/', 'abc-123') === 1, 'positive match failed');
    demo_assert(preg_match('/^[a-z]+-[0-9]{3}$/', 'ABC-123') === 0, 'negative match unexpected');
    return 'match+reject ok';
}

function demo_b05()
{
    if (!function_exists('gzencode')) {
        throw new DemoSkip('zlib 扩展未启用');
    }
    $payload = str_repeat('gzip-payload-', 8);
    $packed = gzencode($payload);
    demo_assert(gzdecode($packed) === $payload, 'gzip round-trip mismatch');
    return 'compress+decompress ok';
}

// -------------------------------------------------------------------- 主流程
function demo_main($argv)
{
    $depth = getenv('DEMO_DEPTH');
    $GLOBALS['depth'] = $depth === false ? 0 : (int) $depth;
    if ($GLOBALS['depth'] > MAX_DEPTH) {
        fwrite(STDERR, "FATAL: probe recursion depth {$GLOBALS['depth']} exceeded\n");
        return 3;
    }

    // ⚠️ 模式分派必须先于任何检查执行：探针会自我调用，一旦派错就会指数级派生进程。
    if (count($argv) >= 1) {
        if ($argv[0] === '--echo-child') {
            fwrite(STDOUT, "child-ok\n");
            fwrite(STDERR, "child-err\n");
            return 0;
        }
        if ($argv[0] === '--read-stdin') {
            $line = trim((string) fgets(STDIN));
            fwrite(STDOUT, $line === 'ping' ? "pong\n" : "unexpected:{$line}\n");
            return 0;
        }
        if ($argv[0] === '--argv-probe') {
            $rest = array_slice($argv, 1);
            fwrite(STDOUT, 'argv-probe:' . count($rest) . ':' . implode(':', $rest) . "\n");
            return 0;
        }
        if ($argv[0] === '--count-worker') {
            $file = $argv[1];
            $fh = fopen($file, 'a');
            if ($fh === false) {
                fwrite(STDERR, "cannot open {$file}\n");
                return 4;
            }
            for ($i = 0; $i < 1000; $i++) {
                flock($fh, LOCK_EX);
                fwrite($fh, "x\n");
                flock($fh, LOCK_UN);
            }
            fclose($fh);
            return 0;
        }
    }

    $ids = [
        'A01_argv', 'A02_env', 'A03_streams', 'A04_stdin', 'A05_file_rw', 'A06_file_append',
        'A07_file_binary', 'A08_file_stat', 'A09_dir_ops', 'A10_temp_file', 'A11_unicode',
        'A12_large_io', 'A13_time', 'A14_random', 'A15_container', 'A16_error',
        'A17_subprocess', 'A18_concurrency', 'A19_tcp_loopback',
        'B01_sha256', 'B02_base64', 'B03_json', 'B04_regex', 'B05_gzip',
    ];
    $fns = [
        'demo_a01', 'demo_a02', 'demo_a03', 'demo_a04', 'demo_a05', 'demo_a06',
        'demo_a07', 'demo_a08', 'demo_a09', 'demo_a10', 'demo_a11', 'demo_a12',
        'demo_a13', 'demo_a14', 'demo_a15', 'demo_a16', 'demo_a17', 'demo_a18', 'demo_a19',
        'demo_b01', 'demo_b02', 'demo_b03', 'demo_b04', 'demo_b05',
    ];

    for ($i = 0; $i < count($fns); $i++) {
        demo_run($ids[$i], $fns[$i]);
    }

    fwrite(STDOUT, "SUMMARY php {$GLOBALS['npass']} {$GLOBALS['nfail']} {$GLOBALS['nskip']}\n");
    fflush(STDOUT);
    return $GLOBALS['nfail'] > 0 ? 1 : 0;
}

exit(demo_main(array_slice($argv, 1)));
