// demos 能力探针 —— Go 实现。协议见 demos/SPEC.md。
//
// 模式（照抄 SPEC §4，跨语言必须同名）：
//
//	默认              跑全部检查
//	--echo-child      往 stdout 写 child-ok、往 stderr 写 child-err，exit 0
//	--read-stdin      读一行 stdin，内容为 ping 时回 pong
//	--argv-probe A B  回显 argv（argv-probe:2:A:B）
package main

import (
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"math/rand"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"
	"unicode/utf8"
)

const (
	unicodeS          = "中文-日本語-한국어-🚀"
	unicodeCodepoints = 12
	unicodeBytes      = 31
	largeBytes        = 262144
	sha256Abc         = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
	maxDepth          = 3
)

var (
	depth int
	npass int
	nfail int
	nskip int
)

func record(cid, status, detail string) {
	clean := strings.NewReplacer("\t", " ", "\r", " ", "\n", " ").Replace(detail)
	if len(clean) > 160 {
		clean = clean[:160]
	}
	switch status {
	case "PASS":
		npass++
	case "FAIL":
		nfail++
	default:
		nskip++
	}
	fmt.Fprintf(os.Stdout, "CAP %s %s %s\n", cid, status, clean)
}

func runCheck(cid string, fn func() (string, error)) {
	detail, err := fn()
	if err != nil {
		record(cid, "FAIL", err.Error())
		return
	}
	record(cid, "PASS", detail)
}

// -------------------------------------------------------------- 自我调用

// spawn 启动本程序自身。返回退出码、stdout、stderr。
func spawn(args []string, stdinData string) (int, string, string, error) {
	self, err := os.Executable()
	if err != nil {
		return 0, "", "", err
	}
	cmd := exec.Command(self, args...)
	// 过滤掉继承来的 DEMO_DEPTH，避免重复键在 getenv 语义下取到旧值。
	env := make([]string, 0, len(os.Environ())+1)
	for _, e := range os.Environ() {
		if !strings.HasPrefix(e, "DEMO_DEPTH=") {
			env = append(env, e)
		}
	}
	cmd.Env = append(env, "DEMO_DEPTH="+strconv.Itoa(depth+1))
	cmd.Stdin = strings.NewReader(stdinData)

	var so, se bytes.Buffer
	cmd.Stdout = &so
	cmd.Stderr = &se

	err = cmd.Run()
	if err != nil {
		if ee, ok := err.(*exec.ExitError); ok {
			return ee.ExitCode(), so.String(), se.String(), nil
		}
		return 0, so.String(), se.String(), err
	}
	return 0, so.String(), se.String(), nil
}

// ------------------------------------------------------------------ Tier A
func a01() (string, error) {
	_, out, _, err := spawn([]string{"--argv-probe", "alpha", "beta"}, "")
	if err != nil {
		return "", err
	}
	got := strings.TrimSpace(out)
	if got != "argv-probe:2:alpha:beta" {
		return "", fmt.Errorf("argv 回显不符: %s", got)
	}
	return "2 args round-tripped", nil
}

func a02() (string, error) {
	v := os.Getenv("DEMO_LANG_TAG")
	if v != "demos-capability" {
		return "", fmt.Errorf("DEMO_LANG_TAG=%q", v)
	}
	return "env visible", nil
}

func a03() (string, error) {
	_, out, errOut, err := spawn([]string{"--echo-child"}, "")
	if err != nil {
		return "", err
	}
	if strings.TrimSpace(out) != "child-ok" {
		return "", fmt.Errorf("stdout=%q", strings.TrimSpace(out))
	}
	if !strings.Contains(errOut, "child-err") {
		return "", fmt.Errorf("stderr=%q", strings.TrimSpace(errOut))
	}
	if strings.Contains(out, "child-err") {
		return "", fmt.Errorf("stderr leaked into stdout")
	}
	return "stdout/stderr separated", nil
}

func a04() (string, error) {
	_, out, _, err := spawn([]string{"--read-stdin"}, "ping\n")
	if err != nil {
		return "", err
	}
	if strings.TrimSpace(out) != "pong" {
		return "", fmt.Errorf("reply=%q", strings.TrimSpace(out))
	}
	return "ping->pong", nil
}

func a05() (string, error) {
	if err := os.WriteFile("a05.txt", []byte("hello-io"), 0o644); err != nil {
		return "", err
	}
	raw, err := os.ReadFile("a05.txt")
	if err != nil {
		return "", err
	}
	if string(raw) != "hello-io" {
		return "", fmt.Errorf("read back %q", string(raw))
	}
	return "text round-trip ok", nil
}

func a06() (string, error) {
	if err := os.WriteFile("a06.txt", []byte("a"), 0o644); err != nil {
		return "", err
	}
	fh, err := os.OpenFile("a06.txt", os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		return "", err
	}
	if _, err := fh.WriteString("b"); err != nil {
		fh.Close()
		return "", err
	}
	fh.Close()
	raw, err := os.ReadFile("a06.txt")
	if err != nil {
		return "", err
	}
	if string(raw) != "ab" {
		return "", fmt.Errorf("append 结果 %q", string(raw))
	}
	return "append ok", nil
}

func a07() (string, error) {
	payload := make([]byte, 256)
	for i := range payload {
		payload[i] = byte(i)
	}
	if err := os.WriteFile("a07.bin", payload, 0o644); err != nil {
		return "", err
	}
	got, err := os.ReadFile("a07.bin")
	if err != nil {
		return "", err
	}
	if !bytes.Equal(payload, got) {
		return "", fmt.Errorf("二进制往返不一致 len=%d", len(got))
	}
	return "256 bytes incl 0x00/0xFF", nil
}

func a08() (string, error) {
	if err := os.WriteFile("a08.txt", []byte("statted"), 0o644); err != nil {
		return "", err
	}
	st, err := os.Stat("a08.txt")
	if err != nil {
		return "", err
	}
	if st.Size() != 7 {
		return "", fmt.Errorf("size=%d", st.Size())
	}
	if err := os.Remove("a08.txt"); err != nil {
		return "", err
	}
	if _, err := os.Stat("a08.txt"); !os.IsNotExist(err) {
		return "", fmt.Errorf("remove failed")
	}
	return "size=7 then removed", nil
}

func a09() (string, error) {
	dir := "a09dir"
	os.Remove(filepath.Join(dir, "inner.txt"))
	os.Remove(dir)
	if err := os.Mkdir(dir, 0o755); err != nil {
		return "", err
	}
	if err := os.WriteFile(filepath.Join(dir, "inner.txt"), []byte("x"), 0o644); err != nil {
		return "", err
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return "", err
	}
	found := false
	for _, e := range entries {
		if e.Name() == "inner.txt" {
			found = true
		}
	}
	if !found {
		return "", fmt.Errorf("listing missing inner.txt")
	}
	if err := os.Remove(filepath.Join(dir, "inner.txt")); err != nil {
		return "", err
	}
	if err := os.Remove(dir); err != nil {
		return "", err
	}
	if _, err := os.Stat(dir); !os.IsNotExist(err) {
		return "", fmt.Errorf("rmdir failed")
	}
	return "mkdir/list/rmdir ok", nil
}

func a10() (string, error) {
	fh, err := os.CreateTemp(".", "demos-*.tmp")
	if err != nil {
		return "", err
	}
	path := fh.Name()
	defer os.Remove(path)
	if _, err := fh.WriteString("temp-content"); err != nil {
		fh.Close()
		return "", err
	}
	fh.Close()
	raw, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	if string(raw) != "temp-content" {
		return "", fmt.Errorf("temp read %q", string(raw))
	}
	return "unique temp file", nil
}

func a11() (string, error) {
	if cps := utf8.RuneCountInString(unicodeS); cps != unicodeCodepoints {
		return "", fmt.Errorf("codepoints=%d", cps)
	}
	if n := len(unicodeS); n != unicodeBytes {
		return "", fmt.Errorf("bytes=%d", n)
	}
	if err := os.WriteFile("a11.txt", []byte(unicodeS), 0o644); err != nil {
		return "", err
	}
	raw, err := os.ReadFile("a11.txt")
	if err != nil {
		return "", err
	}
	if string(raw) != unicodeS {
		return "", fmt.Errorf("round-trip mismatch")
	}
	return fmt.Sprintf("%d codepoints / %d bytes", unicodeCodepoints, unicodeBytes), nil
}

func a12() (string, error) {
	payload := []byte(strings.Repeat("abcdefgh", largeBytes/8))
	if len(payload) != largeBytes {
		return "", fmt.Errorf("payload len=%d", len(payload))
	}
	if err := os.WriteFile("a12.bin", payload, 0o644); err != nil {
		return "", err
	}
	got, err := os.ReadFile("a12.bin")
	if err != nil {
		return "", err
	}
	if len(got) != largeBytes || !bytes.Equal(payload, got) {
		return "", fmt.Errorf("len=%d", len(got))
	}
	return fmt.Sprintf("%d bytes ok", largeBytes), nil
}

func a13() (string, error) {
	ms := time.Now().UnixMilli()
	if ms < 1577836800000 {
		return "", fmt.Errorf("epoch=%d", ms)
	}
	t0 := time.Now()
	time.Sleep(50 * time.Millisecond)
	delta := time.Since(t0).Seconds() * 1000
	if delta < 40 {
		return "", fmt.Errorf("sleep 只测到 %.1f ms", delta)
	}
	return fmt.Sprintf("sleep %.0fms", delta), nil
}

func a14() (string, error) {
	// math/rand 的全局源在 Go 1.20+ 自动随机播种，这里不需要再 Seed。
	v := rand.Intn(1000)
	if v < 0 || v >= 1000 {
		return "", fmt.Errorf("out of range %d", v)
	}
	return "in [0,1000)", nil
}

func a15() (string, error) {
	data := []int{5, 3, 9, 1, 7, 3}
	sort.Ints(data)
	want := []int{1, 3, 3, 5, 7, 9}
	for i := range want {
		if data[i] != want[i] {
			return "", fmt.Errorf("sorted=%v", data)
		}
	}
	m := map[string]int{"a": 1}
	m["b"] = 2
	if m["a"] != 1 || m["b"] != 2 || len(m) != 2 {
		return "", fmt.Errorf("map broken")
	}
	return "sort + map ok", nil
}

func a16() (string, error) {
	_, err := os.ReadFile("definitely-missing-file-xyz")
	if err == nil {
		return "", fmt.Errorf("no error raised for missing file")
	}
	return fmt.Sprintf("%T caught", err), nil
}

func a17() (string, error) {
	code, out, _, err := spawn([]string{"--echo-child"}, "")
	if err != nil {
		return "", err
	}
	if code != 0 {
		return "", fmt.Errorf("child exit=%d", code)
	}
	if strings.TrimSpace(out) != "child-ok" {
		return "", fmt.Errorf("child stdout=%q", strings.TrimSpace(out))
	}
	return "child exit=0, stdout captured", nil
}

func a18() (string, error) {
	var total int64
	var wg sync.WaitGroup
	for i := 0; i < 2; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 1000; j++ {
				atomic.AddInt64(&total, 1)
			}
		}()
	}
	wg.Wait()
	if total != 2000 {
		return "", fmt.Errorf("total=%d", total)
	}
	return "2 goroutines -> 2000", nil
}

func a19() (string, error) {
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return "", err
	}
	defer ln.Close()
	addr := ln.Addr().String()

	type res struct {
		recv string
		err  error
	}
	ch := make(chan res, 1)
	go func() {
		conn, err := ln.Accept()
		if err != nil {
			ch <- res{"", err}
			return
		}
		defer conn.Close()
		buf := make([]byte, 64)
		n, err := conn.Read(buf)
		if err != nil {
			ch <- res{"", err}
			return
		}
		recv := string(buf[:n])
		if _, err := conn.Write([]byte("tcp-pong")); err != nil {
			ch <- res{recv, err}
			return
		}
		ch <- res{recv, nil}
	}()

	conn, err := net.Dial("tcp", addr)
	if err != nil {
		return "", err
	}
	defer conn.Close()
	if _, err := conn.Write([]byte("tcp-ping")); err != nil {
		return "", err
	}
	buf := make([]byte, 64)
	n, err := conn.Read(buf)
	if err != nil {
		return "", err
	}
	reply := string(buf[:n])

	r := <-ch
	if r.err != nil {
		return "", r.err
	}
	if r.recv != "tcp-ping" || reply != "tcp-pong" {
		return "", fmt.Errorf("recv=%q reply=%q", r.recv, reply)
	}
	return "loopback send/recv ok", nil
}

// ------------------------------------------------------------------ Tier B
func b01() (string, error) {
	sum := sha256.Sum256([]byte("abc"))
	if got := hex.EncodeToString(sum[:]); got != sha256Abc {
		return "", fmt.Errorf("sha256=%s", got)
	}
	return "sha256(abc) ok", nil
}

func b02() (string, error) {
	enc := base64.StdEncoding.EncodeToString([]byte("abc"))
	if enc != "YWJj" {
		return "", fmt.Errorf("b64=%s", enc)
	}
	dec, err := base64.StdEncoding.DecodeString(enc)
	if err != nil {
		return "", err
	}
	if string(dec) != "abc" {
		return "", fmt.Errorf("b64 decode mismatch")
	}
	return "encode+decode ok", nil
}

func b03() (string, error) {
	obj := map[string]any{"k": []int{1, 2, 3}, "n": "v"}
	text, err := json.Marshal(obj)
	if err != nil {
		return "", err
	}
	var back map[string]any
	if err := json.Unmarshal(text, &back); err != nil {
		return "", err
	}
	arr, ok := back["k"].([]any)
	if !ok || len(arr) != 3 || back["n"] != "v" {
		return "", fmt.Errorf("round-trip %s", string(text))
	}
	return "serialize+parse ok", nil
}

func b04() (string, error) {
	re := regexp.MustCompile(`^[a-z]+-[0-9]{3}$`)
	if !re.MatchString("abc-123") {
		return "", fmt.Errorf("positive match failed")
	}
	if re.MatchString("ABC-123") {
		return "", fmt.Errorf("negative match unexpected")
	}
	return "match+reject ok", nil
}

func b05() (string, error) {
	payload := []byte(strings.Repeat("gzip-payload-", 8))
	var buf bytes.Buffer
	zw := gzip.NewWriter(&buf)
	if _, err := zw.Write(payload); err != nil {
		return "", err
	}
	if err := zw.Close(); err != nil {
		return "", err
	}
	zr, err := gzip.NewReader(bytes.NewReader(buf.Bytes()))
	if err != nil {
		return "", err
	}
	back, err := io.ReadAll(zr)
	if err != nil {
		return "", err
	}
	zr.Close()
	if !bytes.Equal(payload, back) {
		return "", fmt.Errorf("gzip round-trip mismatch")
	}
	return "compress+decompress ok", nil
}

// -------------------------------------------------------------------- 主流程
func main() {
	if d, err := strconv.Atoi(os.Getenv("DEMO_DEPTH")); err == nil {
		depth = d
	}
	if depth > maxDepth {
		fmt.Fprintf(os.Stderr, "FATAL: probe recursion depth %d exceeded\n", depth)
		os.Exit(3)
	}

	// ⚠️ 模式分派必须先于任何检查执行：探针会自我调用，一旦派错就会指数级派生进程。
	argv := os.Args[1:]
	if len(argv) >= 1 {
		switch argv[0] {
		case "--echo-child":
			fmt.Fprint(os.Stdout, "child-ok\n")
			fmt.Fprint(os.Stderr, "child-err\n")
			os.Exit(0)
		case "--read-stdin":
			var line string
			fmt.Fscanln(os.Stdin, &line)
			if line == "ping" {
				fmt.Fprint(os.Stdout, "pong\n")
			} else {
				fmt.Fprintf(os.Stdout, "unexpected:%s\n", line)
			}
			os.Exit(0)
		case "--argv-probe":
			rest := argv[1:]
			fmt.Fprintf(os.Stdout, "argv-probe:%d:%s\n", len(rest), strings.Join(rest, ":"))
			os.Exit(0)
		}
	}

	ids := []string{
		"A01_argv", "A02_env", "A03_streams", "A04_stdin", "A05_file_rw", "A06_file_append",
		"A07_file_binary", "A08_file_stat", "A09_dir_ops", "A10_temp_file", "A11_unicode",
		"A12_large_io", "A13_time", "A14_random", "A15_container", "A16_error",
		"A17_subprocess", "A18_concurrency", "A19_tcp_loopback",
		"B01_sha256", "B02_base64", "B03_json", "B04_regex", "B05_gzip",
	}
	fns := []func() (string, error){
		a01, a02, a03, a04, a05, a06, a07, a08, a09, a10, a11, a12, a13, a14, a15, a16, a17, a18, a19,
		b01, b02, b03, b04, b05,
	}

	for i := range fns {
		runCheck(ids[i], fns[i])
	}

	fmt.Fprintf(os.Stdout, "SUMMARY go %d %d %d\n", npass, nfail, nskip)
	if nfail > 0 {
		os.Exit(1)
	}
}
