sudo ip netns add ns1
sudo ip netns add ns2
sudo ip link add veth1 type veth peer name veth2
sudo ip link set veth1 netns ns1
sudo ip link set veth2 netns ns2
sudo ip netns exec ns1 ip link set veth1 up
sudo ip netns exec ns2 ip link set veth2 up
echo "namespaces created"
sudo ip netns exec ns1 ip addr add fd00:1::1/64 dev veth1
sudo ip netns exec ns2 ip addr add fd00:1::2/64 dev veth2
sudo ip netns exec ns1 ip tuntap add mode tap dev tap0
sudo ip netns exec ns1 ip link set up dev tap0
sudo ip netns exec ns2 ip tuntap add mode tap dev tap0
sudo ip netns exec ns2 ip link set up dev tap0
sudo ip netns exec ns1 ip addr add 192.168.1.1/24 dev tap0
sudo ip netns exec ns2 ip addr add 192.168.1.2/24 dev tap0
sudo screen -dmS test1 ip netns exec ns1 ./target/debug/etherip -s fd00:1::1 -d fd00:1::2 --device-name=tap0
sudo screen -dmS test2 ip netns exec ns2 ./target/debug/etherip -s fd00:1::2 -d fd00:1::1 --device-name=tap0