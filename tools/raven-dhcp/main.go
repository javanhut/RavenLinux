package main

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/binary"
	"errors"
	"flag"
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"time"
)

const (
	dhcpClientPort = 68
	dhcpServerPort = 67

	dhcpOpRequest = 1
	dhcpOpReply   = 2

	dhcpHTypeEthernet = 1

	dhcpMagicCookie = 0x63825363
)

const (
	optPad       = 0
	optSubnet    = 1
	optRouter    = 3
	optDNS       = 6
	optHostname  = 12
	optReqIP     = 50
	optLeaseTime = 51
	optMsgType   = 53
	optServerID  = 54
	optParamList = 55
	optMaxSize   = 57
	optClientID  = 61
	optEnd       = 255
)

const (
	msgDiscover = 1
	msgOffer    = 2
	msgRequest  = 3
	msgDecline  = 4
	msgAck      = 5
	msgNak      = 6
	msgRelease  = 7
	msgInform   = 8
)

type dhcpPacket struct {
	YiAddr  net.IP
	Options map[byte][]byte
}

func main() {
	var iface string
	var timeoutSeconds int
	var resolvConfPath string
	var all bool
	var quiet bool

	flag.StringVar(&iface, "i", "", "Interface to configure (e.g. eth0)")
	flag.StringVar(&iface, "interface", "", "Interface to configure (e.g. eth0)")
	flag.IntVar(&timeoutSeconds, "t", 10, "Timeout in seconds")
	flag.IntVar(&timeoutSeconds, "timeout", 10, "Timeout in seconds")
	flag.StringVar(&resolvConfPath, "resolv-conf", "/etc/resolv.conf", "Path to write resolv.conf")
	flag.BoolVar(&all, "all", false, "Configure all non-loopback ethernet interfaces")
	flag.BoolVar(&quiet, "q", false, "Quiet (only print errors)")
	flag.BoolVar(&quiet, "quiet", false, "Quiet (only print errors)")
	flag.Parse()

	timeout := time.Duration(timeoutSeconds) * time.Second
	if timeout <= 0 {
		fatalf("invalid timeout: %d", timeoutSeconds)
	}

	if all {
		if err := runAll(timeout, resolvConfPath, quiet); err != nil {
			fatalf("%v", err)
		}
		return
	}

	if iface == "" {
		fatalf("missing -i/--interface (or use --all)")
	}

	if err := runOne(iface, timeout, resolvConfPath, quiet); err != nil {
		fatalf("%s: %v", iface, err)
	}
}

func runAll(timeout time.Duration, resolvConfPath string, quiet bool) error {
	ifaces, err := net.Interfaces()
	if err != nil {
		return err
	}

	var targets []string
	for _, ifi := range ifaces {
		if ifi.Name == "lo" {
			continue
		}
		// Only attempt ethernet-like interfaces by name to avoid touching bridges/tunnels.
		if !(strings.HasPrefix(ifi.Name, "e") || strings.HasPrefix(ifi.Name, "en")) {
			continue
		}
		if len(ifi.HardwareAddr) != 6 {
			continue
		}
		targets = append(targets, ifi.Name)
	}
	if len(targets) == 0 {
		return errors.New("no suitable interfaces found")
	}

	var firstErr error
	for _, name := range targets {
		if err := runOne(name, timeout, resolvConfPath, quiet); err != nil && firstErr == nil {
			firstErr = fmt.Errorf("%s: %w", name, err)
		}
	}
	return firstErr
}

func runOne(iface string, timeout time.Duration, resolvConfPath string, quiet bool) error {
	if err := ipLinkUp(iface); err != nil && !quiet {
		fmt.Fprintf(os.Stderr, "raven-dhcp: %s: failed to set link up: %v\n", iface, err)
	}

	ifi, err := net.InterfaceByName(iface)
	if err != nil {
		return err
	}
	if len(ifi.HardwareAddr) != 6 {
		return fmt.Errorf("unsupported hardware address length: %d", len(ifi.HardwareAddr))
	}

	xid, err := randU32()
	if err != nil {
		return err
	}

	conn, err := listenDHCP(iface)
	if err != nil {
		return err
	}
	defer conn.Close()

	if err := sendDiscover(conn, ifi.HardwareAddr, xid); err != nil {
		return err
	}

	deadline := time.Now().Add(timeout)
	offer, err := waitFor(conn, xid, msgOffer, deadline)
	if err != nil {
		return err
	}

	serverID, ok := ipv4FromOpt(offer.Options[optServerID])
	if !ok {
		return errors.New("missing DHCP server identifier")
	}

	if err := sendRequest(conn, ifi.HardwareAddr, xid, offer.YiAddr, serverID); err != nil {
		return err
	}

	ack, err := waitFor(conn, xid, msgAck, deadline)
	if err != nil {
		return err
	}

	cfg, err := configFromAck(ack)
	if err != nil {
		return err
	}

	if err := applyConfig(iface, cfg); err != nil {
		return err
	}

	if resolvConfPath != "" && len(cfg.DNS) > 0 {
		if err := writeResolvConf(resolvConfPath, cfg.DNS); err != nil && !quiet {
			fmt.Fprintf(os.Stderr, "raven-dhcp: %s: failed to write %s: %v\n", iface, resolvConfPath, err)
		}
	}

	if !quiet {
		fmt.Printf("%s: leased %s/%d", iface, cfg.IP.String(), cfg.Prefix)
		if cfg.Gateway != nil {
			fmt.Printf(" gw %s", cfg.Gateway.String())
		}
		fmt.Println()
	}

	return nil
}

type leaseConfig struct {
	IP      net.IP
	Prefix  int
	Gateway net.IP
	DNS     []net.IP
}

func configFromAck(pkt dhcpPacket) (leaseConfig, error) {
	ip := pkt.YiAddr.To4()
	if ip == nil {
		return leaseConfig{}, errors.New("no IPv4 address in ACK")
	}

	prefix := 24
	if mask, ok := ipv4FromOpt(pkt.Options[optSubnet]); ok {
		if p, ok := prefixFromMask(mask); ok {
			prefix = p
		}
	}

	var gateway net.IP
	if r := pkt.Options[optRouter]; len(r) >= 4 {
		gateway = net.IPv4(r[0], r[1], r[2], r[3]).To4()
	}

	var dns []net.IP
	if d := pkt.Options[optDNS]; len(d) >= 4 {
		for i := 0; i+3 < len(d); i += 4 {
			dns = append(dns, net.IPv4(d[i], d[i+1], d[i+2], d[i+3]).To4())
		}
	}

	return leaseConfig{
		IP:      ip,
		Prefix:  prefix,
		Gateway: gateway,
		DNS:     dns,
	}, nil
}

func applyConfig(iface string, cfg leaseConfig) error {
	_ = runCmd("ip", "addr", "flush", "dev", iface)
	if err := ipLinkUp(iface); err != nil {
		return err
	}
	if err := runCmd("ip", "addr", "add", fmt.Sprintf("%s/%d", cfg.IP.String(), cfg.Prefix), "dev", iface); err != nil {
		return err
	}
	if cfg.Gateway != nil {
		if err := runCmd("ip", "route", "replace", "default", "via", cfg.Gateway.String(), "dev", iface); err != nil {
			return err
		}
	}
	return nil
}

func writeResolvConf(path string, servers []net.IP) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	var b strings.Builder
	b.WriteString("# Generated by raven-dhcp\n")
	for _, ip := range servers {
		if ip4 := ip.To4(); ip4 != nil {
			b.WriteString("nameserver ")
			b.WriteString(ip4.String())
			b.WriteString("\n")
		}
	}
	return os.WriteFile(path, []byte(b.String()), 0o644)
}

func ipLinkUp(iface string) error {
	return runCmd("ip", "link", "set", "dev", iface, "up")
}

func sendDiscover(conn *net.UDPConn, mac net.HardwareAddr, xid uint32) error {
	pkt, err := buildPacket(mac, xid, msgDiscover, nil, nil)
	if err != nil {
		return err
	}
	_, err = conn.WriteToUDP(pkt, &net.UDPAddr{IP: net.IPv4bcast, Port: dhcpServerPort})
	return err
}

func sendRequest(conn *net.UDPConn, mac net.HardwareAddr, xid uint32, requestedIP net.IP, serverID net.IP) error {
	pkt, err := buildPacket(mac, xid, msgRequest, requestedIP, serverID)
	if err != nil {
		return err
	}
	_, err = conn.WriteToUDP(pkt, &net.UDPAddr{IP: net.IPv4bcast, Port: dhcpServerPort})
	return err
}

func waitFor(conn *net.UDPConn, xid uint32, wantType byte, deadline time.Time) (dhcpPacket, error) {
	buf := make([]byte, 1500)
	for time.Now().Before(deadline) {
		_ = conn.SetReadDeadline(time.Now().Add(750 * time.Millisecond))
		n, _, err := conn.ReadFromUDP(buf)
		if err != nil {
			if errors.Is(err, os.ErrDeadlineExceeded) || isNetTimeout(err) {
				continue
			}
			return dhcpPacket{}, err
		}
		pkt, ok := parsePacket(buf[:n], xid)
		if !ok {
			continue
		}
		if mt, ok := pkt.Options[optMsgType]; ok && len(mt) == 1 && mt[0] == wantType {
			return pkt, nil
		}
	}
	return dhcpPacket{}, fmt.Errorf("timed out waiting for DHCP message type %d", wantType)
}

func isNetTimeout(err error) bool {
	type t interface{ Timeout() bool }
	if e, ok := err.(t); ok {
		return e.Timeout()
	}
	return false
}

func parsePacket(b []byte, wantXID uint32) (dhcpPacket, bool) {
	// Minimum DHCP packet is 240 bytes (BOOTP + cookie).
	if len(b) < 240 {
		return dhcpPacket{}, false
	}
	if b[0] != dhcpOpReply || b[1] != dhcpHTypeEthernet || b[2] != 6 {
		return dhcpPacket{}, false
	}

	xid := binary.BigEndian.Uint32(b[4:8])
	if xid != wantXID {
		return dhcpPacket{}, false
	}

	cookie := binary.BigEndian.Uint32(b[236:240])
	if cookie != dhcpMagicCookie {
		return dhcpPacket{}, false
	}

	yiaddr := net.IPv4(b[16], b[17], b[18], b[19]).To4()
	opts := parseOptions(b[240:])

	return dhcpPacket{YiAddr: yiaddr, Options: opts}, true
}

func parseOptions(b []byte) map[byte][]byte {
	opts := make(map[byte][]byte)
	for i := 0; i < len(b); {
		code := b[i]
		i++
		switch code {
		case optPad:
			continue
		case optEnd:
			return opts
		default:
			if i >= len(b) {
				return opts
			}
			l := int(b[i])
			i++
			if i+l > len(b) || l < 0 {
				return opts
			}
			if _, exists := opts[code]; !exists {
				opts[code] = append([]byte(nil), b[i:i+l]...)
			}
			i += l
		}
	}
	return opts
}

func buildPacket(mac net.HardwareAddr, xid uint32, msgType byte, requestedIP net.IP, serverID net.IP) ([]byte, error) {
	if len(mac) != 6 {
		return nil, errors.New("invalid MAC length")
	}
	h := make([]byte, 240)
	h[0] = dhcpOpRequest
	h[1] = dhcpHTypeEthernet
	h[2] = 6
	binary.BigEndian.PutUint32(h[4:8], xid)
	// Broadcast flag.
	binary.BigEndian.PutUint16(h[10:12], 0x8000)
	copy(h[28:34], mac)
	binary.BigEndian.PutUint32(h[236:240], dhcpMagicCookie)

	var opts bytes.Buffer
	addOpt(&opts, optMsgType, []byte{msgType})
	addOpt(&opts, optClientID, append([]byte{dhcpHTypeEthernet}, mac...))

	if host, err := os.Hostname(); err == nil && host != "" {
		addOpt(&opts, optHostname, []byte(host))
	}

	addOpt(&opts, optMaxSize, []byte{0x02, 0x40}) // 576
	addOpt(&opts, optParamList, []byte{
		optSubnet, optRouter, optDNS, optLeaseTime,
	})

	if requestedIP != nil {
		if ip := requestedIP.To4(); ip != nil {
			addOpt(&opts, optReqIP, []byte(ip))
		}
	}
	if serverID != nil {
		if ip := serverID.To4(); ip != nil {
			addOpt(&opts, optServerID, []byte(ip))
		}
	}

	opts.WriteByte(optEnd)
	return append(h, opts.Bytes()...), nil
}

func addOpt(b *bytes.Buffer, code byte, value []byte) {
	if len(value) > 255 {
		value = value[:255]
	}
	b.WriteByte(code)
	b.WriteByte(byte(len(value)))
	b.Write(value)
}

func listenDHCP(iface string) (*net.UDPConn, error) {
	lc := net.ListenConfig{
		Control: func(network, address string, c syscall.RawConn) error {
			var ctrlErr error
			if err := c.Control(func(fd uintptr) {
				_ = syscall.SetsockoptInt(int(fd), syscall.SOL_SOCKET, syscall.SO_REUSEADDR, 1)
				_ = syscall.SetsockoptInt(int(fd), syscall.SOL_SOCKET, syscall.SO_BROADCAST, 1)
				_ = syscall.SetsockoptString(int(fd), syscall.SOL_SOCKET, syscall.SO_BINDTODEVICE, iface)
			}); err != nil {
				ctrlErr = err
			}
			return ctrlErr
		},
	}

	pc, err := lc.ListenPacket(context.Background(), "udp4", fmt.Sprintf("0.0.0.0:%d", dhcpClientPort))
	if err != nil {
		return nil, err
	}
	conn, ok := pc.(*net.UDPConn)
	if !ok {
		_ = pc.Close()
		return nil, errors.New("unexpected packet conn type")
	}
	return conn, nil
}

func ipv4FromOpt(b []byte) (net.IP, bool) {
	if len(b) != 4 {
		return nil, false
	}
	return net.IPv4(b[0], b[1], b[2], b[3]).To4(), true
}

func prefixFromMask(mask net.IP) (int, bool) {
	m := mask.To4()
	if m == nil {
		return 0, false
	}
	ones, bits := net.IPMask(m).Size()
	return ones, bits == 32 && ones >= 0
}

func randU32() (uint32, error) {
	var b [4]byte
	if _, err := rand.Read(b[:]); err != nil {
		return 0, err
	}
	return binary.BigEndian.Uint32(b[:]), nil
}

func runCmd(name string, args ...string) error {
	cmd := exec.Command(name, args...)
	cmd.Stdout = nil
	cmd.Stderr = nil
	return cmd.Run()
}

func fatalf(format string, a ...any) {
	fmt.Fprintf(os.Stderr, "raven-dhcp: "+format+"\n", a...)
	os.Exit(1)
}
