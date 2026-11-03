#!/bin/bash
# Hush AI Microphone edge cases. Run in Terminal on the Mac (microphone permission):
#   open -a Terminal virtual_mic/macos/edge_tests.command
# Results: target/edge-tests.log. The Core Audio restart step needs sudo (killall coreaudiod).
cd "$(dirname "$0")/../.."  # repository root
# Build first: cargo build --release -p hush-core --features devices --example virtual_mic_check
C=./target/release/examples/virtual_mic_check
out=target/edge-tests.log; : > $out
run() { name=$1; shift; printf '%s ' "$name" >> $out; "$@" >> $out 2>&1; echo "exit=$?" >> $out; }
sleep 2
run baseline           $C --seconds 20
run buffer-64          $C --seconds 15 --buffer 64
run buffer-4096        $C --seconds 15 --buffer 4096
run writer-stops       $C --seconds 10 --stop-writer-after 5
run writer-starts-late $C --seconds 10 --start-writer-after 4
# Old audio in the ring, nobody writing: a new recorder must hear silence.
run fill-ring          $C --write-only --seconds 3
run stale-ring         $C --record-only --seconds 5
# Two recording apps at once (one writes), and a second writer is refused.
$C --write-only --seconds 22 > target/edge-writer.log 2>&1 &
W=$!; sleep 2
$C --record-only --expect-tone --seconds 15 > target/edge-client2.log 2>&1 &
R=$!
run client1            $C --record-only --expect-tone --seconds 15
wait $R; printf 'client2 ' >> $out; cat target/edge-client2.log >> $out
run second-writer      $C --write-only --seconds 2
wait $W; printf 'writer ' >> $out; cat target/edge-writer.log >> $out
# Core Audio restarts while the app writes: new recordings still work.
$C --write-only --seconds 20 > target/edge-writer2.log 2>&1 &
W=$!; sleep 3
sudo -n /usr/bin/killall coreaudiod; sleep 4
run after-restart      $C --record-only --expect-tone --seconds 8
wait $W
# Full CPU load (8 busy processes).
for i in 1 2 3 4 5 6 7 8; do yes > /dev/null & done
run cpu-load           $C --seconds 20
kill $(jobs -p | tail -8) 2>/dev/null
echo done >> $out
