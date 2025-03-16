# EtherIP
このプロジェクトはRustを使ってEtherIPを自作するのを目的にしています。

## Sample
```sh
sudo ./etherip --src-addr=192.168.1.35 --dst-addr=192.168.1.34
sudo ip addr add 192.168.11.1/31 dev tap0
```

systemd:
```
[Unit]
Description=EtherIP Tunnel
After=network.target

[Service]
Type=simple
ExecStartPre=/usr/sbin/ip tuntap add mode tap dev tap0
ExecStartPre=/usr/sbin/ip link set up dev tap0
ExecStartPre=/usr/sbin/ip link set dev tap0 master br0
ExecStart=/usr/bin/etherip -d={destination ip} -s={source ip} --device-name=tap0
ExecStopPost=/usr/sbin/ip tuntap del mode tap dev tap0

[Install]
WantedBy=multi-user.target
```