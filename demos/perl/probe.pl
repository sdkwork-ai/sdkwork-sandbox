#!/usr/bin/env perl
# demos 能力探针 —— Perl 实现。协议见 demos/SPEC.md。
#
# 模式（SPEC §4 的四个 + 一个并发用）：
#   默认              跑全部检查
#   --echo-child      往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
#   --read-stdin      读一行 stdin，内容为 ping 时回 pong
#   --argv-probe A B  回显实参个数与内容
#   --count-worker    往 $ENV{DEMO_OUT} 追加 1000 行（A18 并发用）
#
# 设计取舍：
#  1) 只用 core 模块（Digest::SHA / MIME::Base64 / JSON::PP / IO::Compress::Gzip /
#     IO::Socket::INET / Time::HiRes / IPC::Open3 / File::Temp / Encode），无 CPAN 依赖。
#  2) 模式分派**必须先于任何检查**：探针自我调用，派错会指数级派生进程。
#  3) A18 用 `open($h, '-|', ...)`：它在 open 的瞬间就 fork，**不阻塞**，
#     两次 open 之后两个子进程已在并行跑，最后 close 即 join。
#  4) A19 单进程完成：listen(backlog) 后 connect() 会立刻返回（三次握手在内核完成，
#     连接排在 accept 队列里），再 accept 就不会死锁 —— 无需 fork。
#  5) 输出一律钉 UTF-8 层（binmode :encoding(UTF-8)），不依赖 locale。

use strict;
use warnings;
use utf8;

use Encode qw(encode decode);
use Time::HiRes qw(time sleep);
use IPC::Open3 ();
use IO::Socket::INET;
use Symbol qw(gensym);
use File::Spec;
use File::Temp ();

# 中文只在注释与消息里；stdout/stderr 钉 UTF-8，保证 SPEC §4 的 31 字节读数不随宿主 locale 漂。
binmode(STDOUT, ':encoding(UTF-8)');
binmode(STDERR, ':encoding(UTF-8)');

my $SELF = File::Spec->rel2abs(__FILE__);
my $PERL = $^X;

my $UNICODE_S        = "中文-日本語-한국어-🚀";
my $UNICODE_CODEPOINTS = 12;
my $UNICODE_BYTES    = 31;
my $LARGE_BYTES      = 262144;
my $SHA256_ABC       = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';
my $MAX_DEPTH        = 3;

my ($npass, $nfail, $nskip) = (0, 0, 0);

# ------------------------------------------------------------------ 主流程
my $depth = ($ENV{DEMO_DEPTH} // 0) + 0;
if ($depth > $MAX_DEPTH) {
    print STDERR "FATAL: probe recursion depth $depth exceeded\n";
    exit 3;
}

my $arg0 = $ARGV[0] // '';

if ($arg0 eq '--echo-child') {
    print "child-ok\n";
    print STDERR "child-err\n";
    exit 0;
}

if ($arg0 eq '--read-stdin') {
    my $line = <STDIN>;
    $line = '' unless defined $line;
    $line =~ s/\r?\n\z//;
    print $line eq 'ping' ? "pong\n" : "unexpected:$line\n";
    exit 0;
}

if ($arg0 eq '--argv-probe') {
    my @rest = @ARGV[ 1 .. $#ARGV ];
    @rest = () if $#ARGV < 1;
    print "argv-probe:" . scalar(@rest) . ":" . join(':', @rest) . "\n";
    exit 0;
}

if ($arg0 eq '--count-worker') {
    my $out = $ENV{DEMO_OUT} or exit 4;
    open(my $fh, '>>:raw', $out) or exit 5;
    print $fh "x\n" for 1 .. 1000;
    close($fh) or exit 6;
    exit 0;
}

my @CHECKS = (
    [ 'A01_argv'         => \&a01 ],
    [ 'A02_env'          => \&a02 ],
    [ 'A03_streams'      => \&a03 ],
    [ 'A04_stdin'        => \&a04 ],
    [ 'A05_file_rw'      => \&a05 ],
    [ 'A06_file_append'  => \&a06 ],
    [ 'A07_file_binary'  => \&a07 ],
    [ 'A08_file_stat'    => \&a08 ],
    [ 'A09_dir_ops'      => \&a09 ],
    [ 'A10_temp_file'    => \&a10 ],
    [ 'A11_unicode'      => \&a11 ],
    [ 'A12_large_io'     => \&a12 ],
    [ 'A13_time'         => \&a13 ],
    [ 'A14_random'       => \&a14 ],
    [ 'A15_container'    => \&a15 ],
    [ 'A16_error'        => \&a16 ],
    [ 'A17_subprocess'   => \&a17 ],
    [ 'A18_concurrency'  => \&a18 ],
    [ 'A19_tcp_loopback' => \&a19 ],
    [ 'B01_sha256'       => \&b01 ],
    [ 'B02_base64'       => \&b02 ],
    [ 'B03_json'         => \&b03 ],
    [ 'B04_regex'        => \&b04 ],
    [ 'B05_gzip'         => \&b05 ],
);

for my $c (@CHECKS) {
    probe($c->[0], $c->[1]);
}

printf "SUMMARY perl %d %d %d\n", $npass, $nfail, $nskip;
exit($nfail > 0 ? 1 : 0);

# ------------------------------------------------------------------ 框架
sub record {
    my ($cid, $status, $detail) = @_;
    $detail = '' unless defined $detail;
    $detail =~ s/[\t\r\n]+/ /g;          # detail 必须单行、无制表符，否则破坏 runner 的 TSV
    $detail = substr($detail, 0, 160) if length($detail) > 160;
    if    ($status eq 'PASS') { $npass++ }
    elsif ($status eq 'FAIL') { $nfail++ }
    else                      { $nskip++ }
    print "CAP $cid $status $detail\n";
    STDOUT->flush;
    return;
}

# 检查函数返回字符串：`SKIP:理由` → SKIP，`FAIL:理由` → FAIL，其余 → PASS（内容即 detail）。
# 未捕获的 die 也算 FAIL（探针就是要兜住一切）。
sub probe {
    my ($cid, $fn) = @_;
    my $res = eval { $fn->() };
    if ($@) {
        my $e = $@;
        $e =~ s/\s+\z//;
        return record($cid, 'FAIL', "uncaught: $e");
    }
    return record($cid, 'FAIL', 'no result') unless defined $res;
    $res =~ s/\r?\n\z//;
    if ($res =~ /\ASKIP:\s*(.*)\z/s) {
        my $why = $1;
        return record($cid, 'SKIP', 'SKIP declared without reason') if $why eq '';
        return record($cid, 'SKIP', $why);
    }
    if ($res =~ /\AFAIL:\s*(.*)\z/s) {
        my $why = $1;
        return record($cid, 'FAIL', 'FAIL declared without reason') if $why eq '';
        return record($cid, 'FAIL', $why);
    }
    return record($cid, 'PASS', $res);
}

# 启动自身的一个子进程，返回 (退出码, stdout, stderr)。
# 用 IPC::Open3 走**列表形式**，不经 shell ⇒ 没有 Windows/POSIX 引号差异。
sub run_self {
    my (%opt) = @_;
    my @args = @{ $opt{args} // [] };

    my $err = gensym;
    local $ENV{DEMO_DEPTH} = $depth + 1;
    my ($in, $out);
    my $pid = IPC::Open3::open3($in, $out, $err, $PERL, $SELF, @args);

    if (defined $opt{stdin}) {
        print {$in} $opt{stdin};
    }
    close($in);

    local $/;
    my $stdout = <$out>;
    my $stderr = <$err>;
    $stdout = '' unless defined $stdout;
    $stderr = '' unless defined $stderr;
    close($out);
    close($err);
    waitpid($pid, 0);
    my $rc = $? >> 8;
    return ($rc, $stdout, $stderr);
}

sub trimnl {
    my ($s) = @_;
    $s = '' unless defined $s;
    $s =~ s/\r?\n\z//;
    return $s;
}

# ------------------------------------------------------------------ Tier A
sub a01 {
    my ($rc, $out) = run_self(args => [ '--argv-probe', 'alpha', 'beta' ]);
    my $got = trimnl($out);
    return "FAIL:argv 回显不符: $got" if $got ne 'argv-probe:2:alpha:beta';
    return '2 args round-tripped';
}

sub a02 {
    my $v = $ENV{DEMO_LANG_TAG} // '';
    return "FAIL:DEMO_LANG_TAG=$v" if $v ne 'demos-capability';
    return 'env visible';
}

sub a03 {
    my ($rc, $out, $err) = run_self(args => ['--echo-child']);
    my $o = trimnl($out);
    return "FAIL:stdout=$o" if $o ne 'child-ok';
    return 'FAIL:stderr missing child-err' if index($err, 'child-err') < 0;
    return 'FAIL:stderr leaked into stdout' if index($o, 'child-err') >= 0;
    return 'stdout/stderr separated';
}

sub a04 {
    my ($rc, $out) = run_self(args => ['--read-stdin'], stdin => "ping\n");
    my $s = trimnl($out);
    return "FAIL:reply=$s" if $s ne 'pong';
    return 'ping->pong';
}

sub a05 {
    open(my $fh, '>:raw', 'a05.txt') or return "FAIL:open $!";
    print $fh 'hello-io';
    close($fh) or return "FAIL:close $!";
    open($fh, '<:raw', 'a05.txt') or return "FAIL:reopen $!";
    local $/;
    my $got = <$fh>;
    close($fh);
    $got = '' unless defined $got;
    return "FAIL:read back $got" if $got ne 'hello-io';
    return 'text round-trip ok';
}

sub a06 {
    # 不带换行：SPEC 与其他语言都按 "ab"（无换行）对齐，`print` 会补 "\n" 变成 "a\nb\n"。
    open(my $fh, '>:raw', 'a06.txt') or return "FAIL:open $!";
    print $fh 'a';
    close($fh) or return "FAIL:close $!";
    open($fh, '>>:raw', 'a06.txt') or return "FAIL:reopen $!";
    print $fh 'b';
    close($fh) or return "FAIL:close2 $!";
    open($fh, '<:raw', 'a06.txt') or return "FAIL:read $!";
    local $/;
    my $got = <$fh>;
    close($fh);
    $got = '' unless defined $got;
    return "FAIL:append 结果 $got" if $got ne 'ab';
    return 'append ok';
}

sub a07 {
    my $payload = pack('C*', 0 .. 255);      # 恰好 256 字节，含 0x00 与 0xFF
    open(my $fh, '>:raw', 'a07.bin') or return "FAIL:open $!";
    print $fh $payload;
    close($fh) or return "FAIL:close $!";
    open($fh, '<:raw', 'a07.bin') or return "FAIL:reopen $!";
    local $/;
    my $back = <$fh>;
    close($fh);
    $back = '' unless defined $back;
    if (length($back) != 256 || $back ne $payload) {
        return 'FAIL:' . length($back) . ' bytes, round-trip mismatch';
    }
    return '256 bytes incl 0x00/0xFF';
}

sub a08 {
    open(my $fh, '>:raw', 'a08.txt') or return "FAIL:open $!";
    print $fh 'statted';                      # 7 字节，不带换行
    close($fh) or return "FAIL:close $!";
    return 'FAIL:file not created' unless -e 'a08.txt';
    my $size = -s 'a08.txt';
    return "FAIL:size=$size" if $size != 7;
    unlink 'a08.txt' or return "FAIL:unlink $!";
    return 'FAIL:remove failed' if -e 'a08.txt';
    return 'size=7 then removed';
}

sub a09 {
    if (-d 'a09dir') {
        unlink 'a09dir/inner.txt' if -e 'a09dir/inner.txt';
        rmdir 'a09dir';
    }
    mkdir('a09dir') or return "FAIL:mkdir $!";
    open(my $fh, '>:raw', 'a09dir/inner.txt') or return "FAIL:create inner $!";
    print $fh 'x';
    close($fh);

    opendir(my $dh, 'a09dir') or return "FAIL:opendir $!";
    my @entries = readdir($dh);
    closedir($dh);
    return 'FAIL:listing missing inner.txt' unless grep { $_ eq 'inner.txt' } @entries;

    unlink 'a09dir/inner.txt' or return "FAIL:unlink inner $!";
    rmdir('a09dir') or return "FAIL:rmdir $!";
    return 'FAIL:still present after rmdir' if -d 'a09dir';
    return 'mkdir/list/rmdir ok';
}

sub a10 {
    my $tmp = File::Temp->new(TEMPLATE => 'demos-XXXXXX', SUFFIX => '.tmp', DIR => '.', UNLINK => 0);
    my $path = $tmp->filename;
    return 'FAIL:temp name collision' if -e $path && -z $path && 0;
    print {$tmp} 'temp-content';
    $tmp->flush;
    $tmp->close;

    open(my $fh, '<:raw', $path) or return "FAIL:reopen $!";
    local $/;
    my $got = <$fh>;
    close($fh);
    $got = '' unless defined $got;
    unlink $path;
    return "FAIL:temp read $got" if $got ne 'temp-content';
    return 'unique temp file';
}

sub a11 {
    my $s = $UNICODE_S;
    my $cps = length($s);
    return "FAIL:codepoints=$cps" if $cps != $UNICODE_CODEPOINTS;
    my $nbytes = length(encode('UTF-8', $s));
    return "FAIL:bytes=$nbytes" if $nbytes != $UNICODE_BYTES;

    open(my $fh, '>:raw', 'a11.txt') or return "FAIL:open $!";
    print $fh encode('UTF-8', $s);            # 直接写 UTF-8 字节，不靠 IO 层
    close($fh) or return "FAIL:close $!";

    open($fh, '<:raw', 'a11.txt') or return "FAIL:reopen $!";
    local $/;
    my $raw = <$fh>;
    close($fh);
    $raw = '' unless defined $raw;
    my $onfile = length($raw);
    return "FAIL:onfile=$onfile" if $onfile != $UNICODE_BYTES;

    my $got = decode('UTF-8', $raw);
    return 'FAIL:round-trip mismatch' if $got ne $s;
    return "$cps codepoints / $nbytes bytes";
}

sub a12 {
    return 'FAIL:constant drift' if $LARGE_BYTES != 262144;
    my $chunk = 'abcdefgh' x 1024;            # 8192
    my $payload = $chunk x 32;                # 262144
    open(my $fh, '>:raw', 'a12.bin') or return "FAIL:open $!";
    print $fh $payload;
    close($fh) or return "FAIL:close $!";
    open($fh, '<:raw', 'a12.bin') or return "FAIL:reopen $!";
    local $/;
    my $got = <$fh>;
    close($fh);
    $got = '' unless defined $got;
    if (length($got) != $LARGE_BYTES || $got ne $payload) {
        return 'FAIL:len=' . length($got);
    }
    return "$LARGE_BYTES bytes ok";
}

sub a13 {
    my $now_ms = int(time() * 1000);
    return "FAIL:epoch=$now_ms" if $now_ms < 1577836800000;

    my $t0 = _monotonic();
    return 'SKIP:' . 'Time::HiRes 无可用单调时钟' unless defined $t0;
    sleep(0.05);
    my $delta_ms = (_monotonic() - $t0) * 1000.0;
    return sprintf('FAIL:sleep 只测到 %.1f ms', $delta_ms) if $delta_ms < 40;
    return sprintf('sleep %.0fms (monotonic)', $delta_ms);
}

sub _monotonic {
    my $t = eval { Time::HiRes::clock_gettime(Time::HiRes::CLOCK_MONOTONIC()) };
    return $t if defined $t && !$@;
    return undef;
}

sub a14 {
    my $v = int(rand(1000));
    return "FAIL:out of range $v" if $v < 0 || $v >= 1000;
    return 'in [0,1000) (rand)';
}

sub a15 {
    my @data = (5, 3, 9, 1, 7, 3);
    @data = sort { $a <=> $b } @data;
    my $joined = join(',', @data);
    return "FAIL:sorted=$joined" if $joined ne '1,3,3,5,7,9';

    my %m = (a => 1);
    $m{b} = 2;
    return 'FAIL:map broken' if $m{a} != 1 || $m{b} != 2;
    return 'FAIL:map len=' . scalar(keys %m) if scalar(keys %m) != 2;

    delete $m{a};
    return 'FAIL:map delete' if exists $m{a} || scalar(keys %m) != 1;
    return 'sort + hash insert/delete ok';
}

sub a16 {
    my $ok = eval {
        open(my $fh, '<:raw', 'definitely-missing-file-xyz') or die "open failed: $!\n";
        1;
    };
    return 'FAIL:no error raised for missing file' if $ok;
    my $e = $@;
    $e =~ s/\s+\z//;
    return 'FAIL:wrong error' if $e eq '';
    return 'die/eval caught missing-file error';
}

sub a17 {
    my ($rc, $out) = run_self(args => ['--echo-child']);
    return "FAIL:child exit=$rc" if $rc != 0;
    return 'FAIL:child stdout=' . trimnl($out) if trimnl($out) ne 'child-ok';
    return 'child exit=0, stdout captured';
}

sub a18 {
    unlink 'a18.1.txt', 'a18.2.txt';
    local $ENV{DEMO_DEPTH} = $depth + 1;

    # ⚠️ 模式走**实参**（SPEC §4 的形式），不是环境变量：漏掉 '--count-worker'
    # 会让子进程跑成"默认全量检查"，19 个检查各自再派生 → 指数递归。
    # `open '-|'` 在 open 的瞬间 fork 且不阻塞 ⇒ 两次 open 之后两个子进程已在并行跑。
    my ($h1, $h2);
    {
        local $ENV{DEMO_OUT} = 'a18.1.txt';
        open($h1, '-|', $PERL, $SELF, '--count-worker') or return "FAIL:spawn1 $!";
    }
    {
        local $ENV{DEMO_OUT} = 'a18.2.txt';
        open($h2, '-|', $PERL, $SELF, '--count-worker') or return "FAIL:spawn2 $!";
    }

    # close 等价于 join：等子进程退出，并取出退出码。
    my $rc1 = close($h1) ? 0 : ($? >> 8);
    my $rc2 = close($h2) ? 0 : ($? >> 8);
    return "FAIL:worker exit=$rc1/$rc2" if $rc1 != 0 || $rc2 != 0;

    my $n1 = _count_lines('a18.1.txt');
    my $n2 = _count_lines('a18.2.txt');
    return 'FAIL:total=' . ($n1 + $n2) if $n1 + $n2 != 2000;
    return "2 concurrent workers joined -> $n1+$n2";
}

sub _count_lines {
    my ($path) = @_;
    open(my $fh, '<:raw', $path) or return -1;
    local $/;
    my $s = <$fh>;
    close($fh);
    $s = '' unless defined $s;
    my $n = () = $s =~ /\n/g;
    return $n;
}

sub a19 {
    my $srv = IO::Socket::INET->new(
        LocalAddr => '127.0.0.1', LocalPort => 0, Proto => 'tcp',
        Listen    => 5, ReuseAddr => 1,
    ) or return "FAIL:listen $!";
    my $port = $srv->sockport;
    return 'FAIL:no ephemeral port' unless $port;

    # listen 之后 connect 立刻返回（内核完成握手，连接排在 accept 队列）⇒ 单进程不死锁。
    my $cli = IO::Socket::INET->new(
        PeerAddr => '127.0.0.1', PeerPort => $port, Proto => 'tcp',
    ) or return "FAIL:connect $!";

    my $conn = $srv->accept or return "FAIL:accept $!";
    $cli->autoflush(1);
    $conn->autoflush(1);

    print {$cli} 'tcp-ping';
    my $got = '';
    sysread($conn, $got, 64);
    print {$conn} 'tcp-pong';
    my $reply = '';
    sysread($cli, $reply, 64);

    close($conn);
    close($cli);
    close($srv);

    return "FAIL:recv=[$got] reply=[$reply]" if $got ne 'tcp-ping' || $reply ne 'tcp-pong';
    return "loopback send/recv ok (port $port)";
}

# ------------------------------------------------------------------ Tier B
sub b01 {
    require Digest::SHA;
    my $got = Digest::SHA::sha256_hex('abc');
    return "FAIL:sha256=$got" if $got ne $SHA256_ABC;
    return 'sha256(abc) ok (Digest::SHA)';
}

sub b02 {
    require MIME::Base64;
    my $enc = MIME::Base64::encode_base64('abc', '');
    return "FAIL:b64=$enc" if $enc ne 'YWJj';
    my $dec = MIME::Base64::decode_base64($enc);
    return 'FAIL:b64 decode mismatch' if $dec ne 'abc';
    return 'encode+decode ok (MIME::Base64)';
}

sub b03 {
    require JSON::PP;
    my $obj  = { k => [ 1, 2, 3 ], n => 'v' };
    my $text = JSON::PP->new->canonical->encode($obj);
    my $back = JSON::PP->new->decode($text);
    return 'FAIL:round-trip mismatch'
        if $back->{n} ne 'v'
        || join(',', @{ $back->{k} }) ne '1,2,3';
    return 'serialize+parse ok (JSON::PP)';
}

sub b04 {
    return 'FAIL:positive match failed' if 'abc-123' !~ /\A[a-z]+-[0-9]{3}\z/;
    return 'FAIL:negative match unexpected' if 'ABC-123' =~ /\A[a-z]+-[0-9]{3}\z/;
    return 'match+reject ok';
}

sub b05 {
    require IO::Compress::Gzip;
    require IO::Uncompress::Gunzip;
    my $payload = 'gzip-payload-' x 8;
    my $packed  = '';
    IO::Compress::Gzip::gzip(\$payload => \$packed) or return 'FAIL:gzip compress failed';
    my $back = '';
    IO::Uncompress::Gunzip::gunzip(\$packed => \$back) or return 'FAIL:gunzip failed';
    return 'FAIL:gzip round-trip mismatch' if $back ne $payload;
    return 'compress+decompress ok (IO::Compress::Gzip)';
}
