"""Bounded discovery experiment; announces an address, never credentials."""
import argparse
import ipaddress
import json
import socket
import subprocess
import time
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument('--tailscale', type=Path, required=True)
    parser.add_argument('--endpoint', required=True)
    parser.add_argument('--seconds', type=float, default=15)
    parser.add_argument('--include-self', action='store_true', help='Host/Simulator smoke proof only')
    args = parser.parse_args()
    status = json.loads(subprocess.run([str(args.tailscale), 'status', '--json'],
                                      check=True, capture_output=True, timeout=10).stdout)
    if status.get('BackendState') != 'Running':
        raise RuntimeError('Tailscale is not connected')
    owner = status['Self']['UserID']
    peers = [p for p in (status.get('Peer') or {}).values()
             if p.get('Online') and p.get('UserID') == owner and not p.get('Tags') and p.get('OS') == 'iOS']
    if args.include_self:
        peers.append(status['Self'])
    targets = [ip for peer in peers for ip in peer.get('TailscaleIPs', [])
               if ipaddress.ip_address(ip).version == 4]
    data = json.dumps({'version': 1, 'service': 'loopflow', 'endpoint': args.endpoint}).encode()
    if len(data) > 2048:
        raise ValueError('Offer exceeds the experiment limit')
    print(f'Eligible destinations: {len(targets)}; self included: {args.include_self}')
    if not targets:
        raise SystemExit('No online iOS peers owned by this Tailscale user; no packets sent')
    deadline = time.monotonic() + min(max(args.seconds, 0), 60)
    count = 0
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sender:
        while time.monotonic() < deadline:
            for ip in targets:
                sender.sendto(data, (ip, 47471))
                count += 1
            time.sleep(min(3, max(0, deadline - time.monotonic())))
    print(f'Sent {count} bounded address announcements')


if __name__ == '__main__':
    main()
